# 插件开发指南

Ripple 插件用 WebAssembly（WASM）实现，在 wasmtime 沙箱中运行。插件可注册自定义工具，供 AI 在对话中调用。

## 为什么用 WASM

| 特性 | 说明 |
|------|------|
| 内存隔离 | 插件崩溃不影响主进程 |
| 权限沙箱 | 能力令牌系统，无法越权访问系统 |
| 跨平台 | 一次编译，Windows/macOS/Linux 通用 |
| 语言无关 | Rust、C/C++、AssemblyScript 等均可编译为 WASM |

## 插件结构

```
my-plugin/
├── manifest.json     # 清单：声明工具、权限、配置
├── plugin.wasm       # 编译后的 WASM 模块
└── README.md
```

## Manifest 格式

```json
{
  "id": "weather-plugin",
  "name": "天气查询",
  "version": "1.0.0",
  "description": "查询城市天气",
  "author": "you",
  "wasm_file": "plugin.wasm",
  "tools": [
    {
      "name": "get_weather",
      "description": "Get current weather for a city",
      "parameters": {
        "type": "object",
        "properties": {
          "city": { "type": "string", "description": "City name" },
          "units": { "type": "string", "enum": ["metric", "imperial"], "default": "metric" }
        },
        "required": ["city"]
      }
    }
  ],
  "permissions": ["http:api.openweathermap.org"],
  "config_schema": {
    "type": "object",
    "properties": {
      "api_key": { "type": "string", "description": "OpenWeatherMap API key" }
    },
    "required": ["api_key"]
  }
}
```

## WIT 接口

插件需导出 `tool-handler` 接口，可导入 `host` 接口：

```wit
package ripple:plugin;

interface tool-handler {
    init: func(config: string) -> result<_, string>;
    list-tools: func() -> list<tool-def>;
    execute: func(tool-name: string, arguments: string) -> result<string, string>;
    shutdown: func();
}

interface host {
    http-request: func(method: string, url: string, headers: list<tuple<string,string>>, body: option<string>) -> result<http-response, string>;
    file-read: func(path: string) -> result<list<u8>, string>;
    log: func(level: string, message: string);
}

record tool-def {
    name: string,
    description: string,
    parameters: string,
}

world plugin-world {
    import host;
    export tool-handler;
}
```

## 用 Rust 开发插件

```bash
# 安装 wasm32 target
rustup target add wasm32-wasi

# 用 cargo-component 创建项目
cargo new --lib my-plugin
cd my-plugin
```

`Cargo.toml`:
```toml
[package]
name = "weather-plugin"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.30"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`src/lib.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct WeatherArgs {
    city: String,
    units: Option<String>,
}

#[derive(Serialize)]
struct WeatherResult {
    city: String,
    temp: f64,
    description: String,
}

wit_bindgen::generate!({
    path: "../wit/plugin.wit",
    world: "plugin-world",
});

struct Plugin;

impl Guest for Plugin {
    fn init(_config: String) -> Result<(), String> { Ok(()) }

    fn list_tools() -> Vec<ToolDef> {
        vec![ToolDef {
            name: "get_weather".into(),
            description: "Get current weather for a city".into(),
            parameters: r#"{"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}"#.into(),
        }]
    }

    fn execute(tool_name: String, arguments: String) -> Result<String, String> {
        if tool_name != "get_weather" {
            return Err(format!("Unknown tool: {}", tool_name));
        }
        let args: WeatherArgs = serde_json::from_str(&arguments).map_err(|e| e.to_string())?;

        // 通过 host http-request 调用 API（受 permissions 限制）
        let resp = http_request(
            "GET",
            &format!("https://api.openweathermap.org/data/2.5/weather?q={}&units=metric", args.city),
            &[], None,
        ).map_err(|e| e.to_string())?;

        // 解析并返回
        let result = WeatherResult { city: args.city, temp: 20.0, description: "sunny".into() };
        serde_json::to_string(&result).map_err(|e| e.to_string())
    }

    fn shutdown() {}
}

export_plugin!(Plugin);
```

编译：
```bash
cargo build --target wasm32-wasi --release
# 产物：target/wasm32-wasi/release/weather_plugin.wasm
```

## 能力令牌（权限）

插件在 manifest 声明所需权限，运行时强制校验：

| 能力 | 格式 | 限制方式 |
|------|------|----------|
| HTTP | `http:domain` | 域名匹配 |
| 文件读 | `file:read:path` | 路径前缀匹配 |
| 命令执行 | `shell:command` | 精确匹配 |
| 网络流量 | `network:total:N` | 字节上限 |
| 执行时间 | `time:max:Ms` | 单次调用时限 |

未声明的能力调用会被 host 函数拒绝并返回错误。

## 插件生命周期

```
安装 → 验证 Manifest → 加载 WASM → init(config)
                                     ↓
                              注册 tools 到 tool-registry
                                     ↓
                         对话中 AI 可调用插件工具
                                     ↓
                         禁用/卸载 → shutdown() → 卸载 WASM
```

## 安装

```typescript
// UI 上传 .wasm + manifest.json
invoke("plugin_install", { wasmBytes, manifest })
```

后端校验 manifest 合法性、WASM 模块签名（可选）、权限声明，存入 `plugins` 表与插件目录，加载并注册工具。

## 调试

- 插件 `log()` 输出到应用日志面板（`tracing`）
- `tool_audit_log` 表记录每次工具调用（输入/输出/耗时/状态）
- 开发模式可热重载：修改 `.wasm` 后自动 reload

## 示例插件

`plugins/` 目录提供参考实现：
- `calculator` — 纯计算，无权限需求
- `web-fetch` — 仅需 `http:*` 权限
- `file-indexer` — 需 `file:read` 权限

详见各插件目录 README。
