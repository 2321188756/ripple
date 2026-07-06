//! 插件系统：扫描 plugins/ 目录，加载 manifest，注册工具，执行脚本。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tauri::State;
use rusqlite::params;
use ripple_core::{ToolDefinition, ToolSource};
use crate::state::AppState;

// ---- 插件清单 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub runtime: String,        // "rhai" | "node" | "python" | "shell"
    pub entry: String,          // 入口文件路径（相对 plugins/）
    #[serde(default)]
    pub mode: String,           // "tool" | "transform" | "daemon"
    #[serde(default)]
    pub tools: Vec<PluginTool>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ---- 已加载插件 ----

#[derive(Debug)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
    pub enabled: bool,
}

fn registry() -> &'static Mutex<HashMap<String, LoadedPlugin>> {
    static REG: once_cell::sync::OnceCell<Mutex<HashMap<String, LoadedPlugin>>> = once_cell::sync::OnceCell::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取 plugins/ 目录路径
fn plugins_dir() -> PathBuf {
    std::env::current_exe()
        .ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") { d.pop(); d.pop(); }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") { d.pop(); }
            d.join("plugins")
        })
        .unwrap_or_else(|| PathBuf::from("./plugins"))
}

/// 扫描 plugins/ 目录并加载所有插件
pub fn scan_plugins() -> Vec<String> {
    let dir = plugins_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut loaded = Vec::new();
    let mut registry = registry().lock().unwrap();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() { continue; }
            let manifest_path = plugin_dir.join("manifest.json");
            if !manifest_path.exists() { continue; }

            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => {
                    match serde_json::from_str::<PluginManifest>(&content) {
                        Ok(manifest) => {
                            let name = manifest.name.clone();
                            registry.insert(name.clone(), LoadedPlugin {
                                manifest,
                                dir: plugin_dir,
                                enabled: true,
                            });
                            loaded.push(name);
                        }
                        Err(e) => tracing::warn!(path = %manifest_path.display(), error = %e, "invalid plugin manifest"),
                    }
                }
                Err(e) => tracing::warn!(path = %manifest_path.display(), error = %e, "failed to read manifest"),
            }
        }
    }
    loaded
}

/// api_name → (plugin_name, tool_name) 映射。
/// OpenAI 工具名只允许 [a-zA-Z0-9_-]，不能用 `:`。早期版本用 `plugin_{name}:{tool}` 被 API 拒绝，
/// 改用 sanitized 名 `plugin_{plugin}_{tool}` 并登记映射，执行时反查，避免解析歧义（工具名可能含 `_`）。
fn tool_name_map() -> &'static std::sync::Mutex<std::collections::HashMap<String, (String, String)>> {
    static MAP: once_cell::sync::OnceCell<std::sync::Mutex<std::collections::HashMap<String, (String, String)>>> = once_cell::sync::OnceCell::new();
    MAP.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn sanitize_tool_name_part(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// 将已加载插件的工具注册到工具列表。
/// 工具名用 `plugin_{sanitized_plugin}_{sanitized_tool}`（API 兼容），并登记到 tool_name_map 供执行时反查。
pub fn plugin_tools() -> Vec<ToolDefinition> {
    let registry = registry().lock().unwrap();
    // 每次请求都会调 plugin_tools，先清空旧映射再重建
    let mut map = tool_name_map().lock().unwrap();
    map.clear();
    let mut tools = Vec::new();
    for (name, plugin) in registry.iter() {
        if !plugin.enabled { continue; }
        for pt in &plugin.manifest.tools {
            let base = format!("plugin_{}_{}", sanitize_tool_name_part(name), sanitize_tool_name_part(&pt.name));
            // sanitized 后同名冲突时追加序号
            let mut api_name = base.clone();
            let mut i = 2;
            while map.contains_key(&api_name) {
                api_name = format!("{base}_{i}");
                i += 1;
            }
            map.insert(api_name.clone(), (name.clone(), pt.name.clone()));
            tools.push(ToolDefinition {
                name: api_name,
                description: format!("[{}] {}", name, pt.description),
                parameters: pt.parameters.clone(),
                source: ToolSource::Plugin { plugin_id: name.clone() },
                requires_approval: plugin.manifest.runtime != "rhai",
            });
        }
    }
    drop(map);
    drop(registry);
    tools
}

/// 执行插件工具。api_name 为 plugin_tools 注册的兼容名，经 tool_name_map 反查 (plugin, tool)。
pub async fn exec_by_tool_name(api_name: &str, args: &serde_json::Value) -> Result<String, String> {
    let (plugin_name, tool_name) = {
        let map = tool_name_map().lock().unwrap();
        map.get(api_name).cloned()
            .ok_or_else(|| format!("plugin tool not found: {api_name}"))?
    };
    exec_plugin_tool(&plugin_name, &tool_name, args).await
}

/// 查询某工具是否需要用户审批。plugin_ 开头查 registry（runtime != rhai 即需审批），
/// 其余（内置工具）一律 false（受信）。tool_name_map 在 plugin_tools() 调用时填充。
pub fn tool_requires_approval(name: &str) -> bool {
    if !name.starts_with("plugin_") {
        return false;
    }
    let plugin_name = {
        let map = tool_name_map().lock().unwrap();
        match map.get(name) {
            Some((p, _)) => p.clone(),
            None => return false,
        }
    };
    let registry = registry().lock().unwrap();
    match registry.get(&plugin_name) {
        Some(plugin) => plugin.manifest.runtime != "rhai",
        None => false,
    }
}

// ---- 工具审批：pending 请求注册表 ----

/// request_id → oneshot Sender。后端 emit 审批事件后阻塞等待，前端调 approve_tool_call 回传 (approved, trust_tool)。
fn pending_approvals() -> &'static Mutex<HashMap<String, oneshot::Sender<(bool, bool)>>> {
    static P: once_cell::sync::OnceCell<Mutex<HashMap<String, oneshot::Sender<(bool, bool)>>>> = once_cell::sync::OnceCell::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_pending_approval(id: &str, tx: oneshot::Sender<(bool, bool)>) {
    pending_approvals().lock().unwrap().insert(id.to_string(), tx);
}

pub fn take_pending_approval(id: &str) -> Option<oneshot::Sender<(bool, bool)>> {
    pending_approvals().lock().unwrap().remove(id)
}

// ---- Agent 权限查询 ----

/// Agent 的权限级别：strict / elevated / full。无记录（如 default agent）回退 strict。
pub fn agent_permission_level(conn: &rusqlite::Connection, agent_id: &str) -> String {
    conn.query_row(
        "SELECT permission_level FROM agents WHERE id = ?1",
        [agent_id],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "strict".into())
}

/// 该 Agent 是否已信任此工具（elevated 模式下「信任此工具」积累）。
pub fn is_tool_trusted(conn: &rusqlite::Connection, agent_id: &str, tool_name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM agent_trusted_tools WHERE agent_id = ?1 AND tool_name = ?2",
        params![agent_id, tool_name],
        |_| Ok(true),
    )
    .is_ok()
}

/// 记录信任（用户在 elevated 模式勾选「信任此工具」并批准后调用）。
pub fn add_trusted_tool(conn: &rusqlite::Connection, agent_id: &str, tool_name: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO agent_trusted_tools (agent_id, tool_name, created_at) VALUES (?1, ?2, ?3)",
        params![agent_id, tool_name, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn exec_plugin_tool(plugin_name: &str, tool_name: &str, args: &serde_json::Value) -> Result<String, String> {
    // 取出所需信息后立即释放 registry 锁（std::sync::MutexGuard 非 Send，不能跨 await 持有）
    let (entry_path, code, runtime) = {
        let registry = registry().lock().unwrap();
        let plugin = registry.get(plugin_name).ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        let entry_path = plugin.dir.join(&plugin.manifest.entry);
        let code = std::fs::read_to_string(&entry_path).map_err(|e| format!("read entry: {e}"))?;
        (entry_path, code, plugin.manifest.runtime.clone())
    };

    match runtime.as_str() {
        "rhai" => exec_rhai(&code, args),
        "node" => exec_process("node", &entry_path, args, tool_name).await,
        "python" | "py" => exec_process("python", &entry_path, args, tool_name).await,
        "shell" | "bash" => exec_process("bash", &entry_path, args, tool_name).await,
        other => Err(format!("unsupported runtime: {other}")),
    }
}

// ---- Rhai 脚本引擎 ----

fn exec_rhai(code: &str, _args: &serde_json::Value) -> Result<String, String> {
    // 安全沙箱：只允许字符串操作，直接返回脚本内容
    // 后续可集成真正的 rhai 引擎
    let result = code.trim().to_string();
    if result.is_empty() { Err("empty script".into()) } else { Ok(result) }
}

// ---- 进程执行器 ----

/// 异步执行子进程（tokio::process），不阻塞 tokio 工作线程。
/// 参数通过 stdin 管道传入（JSON + 换行），脚本用 input()/stdin 读取。
/// tool_name 通过 RIPPLE_TOOL 环境变量传入，单脚本能分派多工具（不读则忽略，向后兼容）。
async fn exec_process(cmd: &str, script_path: &PathBuf, args: &serde_json::Value, tool_name: &str) -> Result<String, String> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    let args_json = serde_json::to_string(args).unwrap_or_default();
    let mut child = tokio::process::Command::new(cmd)
        .arg(script_path)
        .env("RIPPLE_TOOL", tool_name)
        // 强制 Python 用 UTF-8 stdio（Windows 默认 GBK，print 非 ASCII 字符会 UnicodeEncodeError，
        // 且后端 from_utf8_lossy 会把 GBK 字节当 UTF-8 解码成乱码）。node/shell 忽略这两个 env，无副作用。
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("exec error: {e}"))?;

    // 通过 stdin 传入 JSON 参数（末尾换行让脚本的 input() 立即返回）
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(args_json.as_bytes()).await.map_err(|e| format!("write stdin: {e}"))?;
        stdin.write_all(b"\n").await.map_err(|e| format!("write stdin: {e}"))?;
    } // stdin drop → 关闭管道 → 脚本 input() 收到 EOF

    let output = child.wait_with_output().await.map_err(|e| format!("wait: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("exit {}: {stderr}", output.status.code().unwrap_or(-1)))
    }
}

// ---- IPC 命令 ----

#[tauri::command]
pub async fn list_plugins() -> Result<Vec<PluginManifest>, String> {
    scan_plugins();
    let registry = registry().lock().unwrap();
    Ok(registry.values().map(|p| p.manifest.clone()).collect())
}

#[tauri::command]
pub async fn toggle_plugin(name: String, enabled: bool) -> Result<(), String> {
    let mut registry = registry().lock().unwrap();
    if let Some(plugin) = registry.get_mut(&name) {
        plugin.enabled = enabled;
        Ok(())
    } else {
        Err(format!("plugin not found: {name}"))
    }
}

/// 读取插件配置（plugins/插件名/config.json）
#[tauri::command]
pub async fn get_plugin_config(name: String) -> Result<serde_json::Value, String> {
    let dir = plugins_dir().join(&name);
    let config_path = dir.join("config.json");
    match std::fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str(&content).map_err(|e| e.to_string()),
        Err(_) => Ok(serde_json::json!({})),
    }
}

/// 写入插件配置
#[tauri::command]
pub async fn set_plugin_config(name: String, config: serde_json::Value) -> Result<(), String> {
    let dir = plugins_dir().join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let config_path = dir.join("config.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content).map_err(|e| e.to_string())
}

/// 执行插件工具（启动 daemon 等）
#[tauri::command]
pub async fn execute_plugin_tool(tool_name: String, args: serde_json::Value) -> Result<String, String> {
    // tool_name 格式: "plugin_name.tool_name"
    let parts: Vec<&str> = tool_name.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err("invalid tool name, expected 'plugin.tool'".into());
    }
    exec_plugin_tool(parts[0], parts[1], &args).await
}

/// 回传工具审批结果（前端审批框调用）。唤醒对应 request_id 的 oneshot，让后端 await 继续。
/// trust_tool=true 表示用户勾选了「信任此工具」（仅 elevated 模式生效，由后端记录）。
#[tauri::command]
pub async fn approve_tool_call(request_id: String, approved: bool, trust_tool: bool) -> Result<(), String> {
    match take_pending_approval(&request_id) {
        Some(tx) => tx.send((approved, trust_tool)).map_err(|_| "approval receiver dropped".into()),
        None => Err("pending approval not found (已超时或已处理)".into()),
    }
}

// ---- Agent 权限管理 IPC ----

/// 读取 Agent 权限级别
#[tauri::command]
pub async fn get_agent_permission_level(state: State<'_, AppState>, agent_id: String) -> Result<String, String> {
    let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
    Ok(agent_permission_level(&conn, &agent_id))
}

/// 设置 Agent 权限级别（strict / elevated / full）
#[tauri::command]
pub async fn set_agent_permission_level(state: State<'_, AppState>, agent_id: String, level: String) -> Result<(), String> {
    if !matches!(level.as_str(), "strict" | "elevated" | "full") {
        return Err(format!("invalid permission level: {level} (expected strict/elevated/full)"));
    }
    let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE agents SET permission_level = ?1, updated_at = ?2 WHERE id = ?3",
        params![level, chrono::Utc::now().to_rfc3339(), agent_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 列出 Agent 已信任的工具
#[tauri::command]
pub async fn list_trusted_tools(state: State<'_, AppState>, agent_id: String) -> Result<Vec<String>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT tool_name FROM agent_trusted_tools WHERE agent_id = ?1 ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([agent_id], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

/// 收回某工具的信任
#[tauri::command]
pub async fn revoke_trust(state: State<'_, AppState>, agent_id: String, tool_name: String) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM agent_trusted_tools WHERE agent_id = ?1 AND tool_name = ?2",
        params![agent_id, tool_name],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
