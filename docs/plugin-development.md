# 插件开发指南

Ripple 插件用脚本实现，按 `runtime` 字段选择运行时：rhai（沙箱）/ node / python / shell（子进程）。插件可注册自定义工具，供 AI 在对话中调用。

> **注意**：不使用 WASM/wasmtime（早期规划，未实现）。插件直接通过 `std::process::Command` 调用外部解释器，或用内置 rhai 沙箱。

## 运行时

| runtime | 执行方式 | 说明 |
|---------|----------|------|
| `rhai` | 内置沙箱（`exec_rhai`） | 目前返回脚本内容本身，后续接入完整 rhai 引擎 |
| `node` | `node <entry> <args_json>` 子进程 | 需系统装 Node.js |
| `python` / `py` | `python <entry> <args_json>` 子进程 | 需系统装 Python |
| `shell` / `bash` | `bash <entry> <args_json>` 子进程 | Shell 脚本 |

工具参数经 JSON 字符串作为命令行参数传入；标准输出为工具结果，非零退出码为错误。

## 插件结构

```
my-plugin/
├── manifest.json     # 清单：声明运行时、工具、配置
├── index.js          # 入口（runtime=node 时）
└── config.json       # 运行时配置（UI 编辑生成，可选）
```

## Manifest 格式

```json
{
  "name": "weather",
  "version": "1.0.0",
  "description": "查询城市天气",
  "author": "you",
  "runtime": "node",
  "entry": "index.js",
  "mode": "tool",
  "tools": [
    {
      "name": "get_weather",
      "description": "Get current weather for a city",
      "parameters": {
        "type": "object",
        "properties": {
          "city": { "type": "string" }
        },
        "required": ["city"]
      }
    }
  ],
  "permissions": [],
  "config_schema": {
    "type": "object",
    "properties": {
      "api_key": { "type": "string", "description": "API key" }
    }
  }
}
```

| 字段 | 说明 |
|------|------|
| `name` | 插件唯一标识（注册表键） |
| `runtime` | `rhai` / `node` / `python` / `shell` |
| `entry` | 入口文件路径（相对 `plugins/<name>/`） |
| `mode` | `tool`（AI 调用）/ `transform`（消息处理）/ `daemon`（后台） |
| `tools` | 注册的工具列表 |
| `permissions` | 权限声明（保留） |
| `config_schema` | 可编辑配置字段，UI 据此生成表单，存 `config.json` |

## 入口脚本示例（node）

```js
// index.js
const args = JSON.parse(process.argv[2] || "{}");
const { city } = args;

if (!city) {
  console.error("missing city");
  process.exit(1);
}
// 调用 API...
const result = { city, temp: 20, description: "sunny" };
console.log(JSON.stringify(result));  // stdout 作为工具结果
```

## 工具注册与执行

### 注册（`plugin_tools`）

```rust
// src/commands/plugins.rs
for (name, plugin) in registry.iter() {
    for pt in &plugin.manifest.tools {
        tools.push(ToolDefinition {
            name: format!("plugin_{}:{}", name, pt.name),  // 注册名带 plugin_ 前缀
            description: format!("[{}] {}", name, pt.description),
            parameters: pt.parameters.clone(),
            source: ToolSource::Plugin { plugin_id: name.clone() },
            requires_approval: plugin.manifest.runtime != "rhai",
        });
    }
}
```

### 执行（`exec_by_tool_name`）

AI 调用工具时，`chat.rs` 匹配 `other if other.starts_with("plugin_")` → `exec_by_tool_name(other, args)`：

```rust
pub fn exec_by_tool_name(full_name: &str, args: &serde_json::Value) -> Result<String, String> {
    // 关键：先剥离 plugin_ 前缀再按 ':' 拆分。
    // 注册表以 {name} 为键，不剥离的话 registry.get("plugin_{name}") 永远 NotFound。
    let stripped = full_name.strip_prefix("plugin_").unwrap_or(full_name);
    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 { return Err(format!("invalid plugin tool name: {full_name}")); }
    exec_plugin_tool(parts[0], parts[1], args)
}
```

> 早期版本未剥离前缀，所有插件工具调用 100% 失败（"plugin not found: plugin_xxx"）。已修。

```rust
pub fn exec_plugin_tool(plugin_name, tool_name, args) -> Result<String, String> {
    let plugin = registry.get(plugin_name).ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
    let entry_path = plugin.dir.join(&plugin.manifest.entry);
    let code = std::fs::read_to_string(&entry_path)?;
    match plugin.manifest.runtime.as_str() {
        "rhai" => exec_rhai(&code, args),
        "node" => exec_process("node", &entry_path, args),
        "python" | "py" => exec_process("python", &entry_path, args),
        "shell" | "bash" => exec_process("bash", &entry_path, args),
        other => Err(format!("unsupported runtime: {other}")),
    }
}
```

> `exec_process` 当前用 `std::process::Command`（阻塞 tokio 线程）。规划改 `tokio::process::Command`（需把 `exec_by_tool_name`/`exec_plugin_tool` 改 async，波及 `chat.rs` 工具分发）。

## 插件配置

`get_plugin_config(name)` 读 `plugins/<name>/config.json`；`set_plugin_config(name, config)` 写入。UI 据 `config_schema` 生成编辑表单。

## 插件生命周期

```
扫描 plugins/ → 加载 manifest.json → 注册到全局 registry（once_cell）
    ↓
plugin_tools() 合入 builtin_tools() → AI 可见 plugin_{name}:{tool}
    ↓
对话中 AI 调用 → exec_by_tool_name → exec_plugin_tool → 子进程/rhai
    ↓
toggle_plugin(name, enabled) 启用/禁用
```

`list_plugins` 命令触发 `scan_plugins()` 重新扫描目录并刷新注册表。

## 调试

- 插件脚本的 stderr 会在工具执行失败时返回给前端（错误横幅）
- `tool_audit_log` 表可记录每次工具调用（输入/输出/耗时/状态）
- 修改插件后重新 `list_plugins` 即可刷新
