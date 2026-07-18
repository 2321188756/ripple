//! 插件系统：扫描 plugins/ 目录，加载 manifest，注册工具，执行脚本。

use crate::state::AppState;
use ripple_core::{ToolDefinition, ToolSource};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;
use tokio::sync::oneshot;

// ---- 插件清单 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub runtime: String, // "rhai" | "node" | "python" | "shell"
    pub entry: String,   // 入口文件路径（相对 plugins/）
    #[serde(default)]
    pub mode: String, // "tool" | "transform" | "daemon"
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

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    #[serde(flatten)]
    pub manifest: PluginManifest,
    pub enabled: bool,
}

const MASKED_VALUE: &str = "••••••••";
const ENABLED_STATE_FILE: &str = ".enabled.json";

// ---- 已加载插件 ----

#[derive(Debug)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
    pub enabled: bool,
}

fn registry() -> &'static Mutex<HashMap<String, LoadedPlugin>> {
    static REG: once_cell::sync::OnceCell<Mutex<HashMap<String, LoadedPlugin>>> =
        once_cell::sync::OnceCell::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取 plugins/ 目录路径
fn plugins_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") {
                d.pop();
                d.pop();
            }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
                d.pop();
            }
            d.join("plugins")
        })
        .unwrap_or_else(|| PathBuf::from("./plugins"))
}

fn valid_plugin_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn plugin_dir_for(name: &str) -> Result<PathBuf, String> {
    if !valid_plugin_name(name) {
        return Err("invalid plugin name".into());
    }
    Ok(plugins_dir().join(name))
}

fn enabled_state_path() -> PathBuf {
    plugins_dir().join(ENABLED_STATE_FILE)
}

fn read_enabled_state() -> HashMap<String, bool> {
    std::fs::read_to_string(enabled_state_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn write_enabled_state(state: &HashMap<String, bool>) -> Result<(), String> {
    let path = enabled_state_path();
    let content = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

fn path_stays_within(base: &std::path::Path, relative: &str) -> bool {
    let path = std::path::Path::new(relative);
    !path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        && base.join(path).starts_with(base)
}

/// 扫描 plugins/ 目录并加载所有插件。
/// 保留持久化开关状态，并清除已被移除的插件，避免重扫后幽灵工具残留。
pub fn scan_plugins() -> Vec<String> {
    let dir = plugins_dir();
    let _ = std::fs::create_dir_all(&dir);
    let enabled_state = read_enabled_state();
    let mut loaded = Vec::new();
    let mut discovered = HashMap::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }
            let manifest_path = plugin_dir.join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match serde_json::from_str::<PluginManifest>(&content) {
                    Ok(manifest) => {
                        let folder_name = plugin_dir
                            .file_name()
                            .and_then(|v| v.to_str())
                            .unwrap_or_default();
                        if !valid_plugin_name(&manifest.name) || manifest.name != folder_name {
                            tracing::warn!(path = %manifest_path.display(), name = %manifest.name, "plugin name must be safe and match its directory");
                            continue;
                        }
                        if !path_stays_within(&plugin_dir, &manifest.entry) {
                            tracing::warn!(path = %manifest_path.display(), entry = %manifest.entry, "plugin entry must be a relative path inside its directory");
                            continue;
                        }
                        let name = manifest.name.clone();
                        discovered.insert(
                            name.clone(),
                            LoadedPlugin {
                                manifest,
                                dir: plugin_dir,
                                enabled: enabled_state.get(&name).copied().unwrap_or(true),
                            },
                        );
                        loaded.push(name);
                    }
                    Err(e) => {
                        tracing::warn!(path = %manifest_path.display(), error = %e, "invalid plugin manifest")
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %manifest_path.display(), error = %e, "failed to read manifest")
                }
            }
        }
    }
    *registry().lock().unwrap() = discovered;
    loaded.sort();
    loaded
}

/// api_name → (plugin_name, tool_name) 映射。
/// OpenAI 工具名只允许 [a-zA-Z0-9_-]，不能用 `:`。早期版本用 `plugin_{name}:{tool}` 被 API 拒绝，
/// 改用 sanitized 名 `plugin_{plugin}_{tool}` 并登记映射，执行时反查，避免解析歧义（工具名可能含 `_`）。
fn tool_name_map() -> &'static std::sync::Mutex<std::collections::HashMap<String, (String, String)>>
{
    static MAP: once_cell::sync::OnceCell<
        std::sync::Mutex<std::collections::HashMap<String, (String, String)>>,
    > = once_cell::sync::OnceCell::new();
    MAP.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn sanitize_tool_name_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
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
        if !plugin.enabled {
            continue;
        }
        for pt in &plugin.manifest.tools {
            let base = format!(
                "plugin_{}_{}",
                sanitize_tool_name_part(name),
                sanitize_tool_name_part(&pt.name)
            );
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
                source: ToolSource::Plugin {
                    plugin_id: name.clone(),
                },
                requires_approval: plugin.manifest.runtime != "rhai",
            });
        }
    }
    drop(map);
    drop(registry);
    tools
}

/// 执行插件工具。api_name 为 plugin_tools 注册的兼容名，经 tool_name_map 反查 (plugin, tool)。
/// api_key/api_base_url 通过 env (RIPPLE_API_KEY/RIPPLE_API_BASE) 传给插件，供需要凭证的工具（image-gen 等）使用。
pub async fn exec_by_tool_name(
    api_name: &str,
    args: &serde_json::Value,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
) -> Result<String, String> {
    let (plugin_name, tool_name) = {
        let map = tool_name_map().lock().unwrap();
        map.get(api_name)
            .cloned()
            .ok_or_else(|| format!("plugin tool not found: {api_name}"))?
    };
    exec_plugin_tool(&plugin_name, &tool_name, args, api_key, api_base_url).await
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
type ApprovalSender = oneshot::Sender<(bool, bool)>;
type PendingApprovals = Mutex<HashMap<String, ApprovalSender>>;

fn pending_approvals() -> &'static PendingApprovals {
    static P: once_cell::sync::OnceCell<PendingApprovals> = once_cell::sync::OnceCell::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_pending_approval(id: &str, tx: oneshot::Sender<(bool, bool)>) {
    pending_approvals()
        .lock()
        .unwrap()
        .insert(id.to_string(), tx);
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
pub fn add_trusted_tool(
    conn: &rusqlite::Connection,
    agent_id: &str,
    tool_name: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO agent_trusted_tools (agent_id, tool_name, created_at) VALUES (?1, ?2, ?3)",
        params![agent_id, tool_name, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn exec_plugin_tool(
    plugin_name: &str,
    tool_name: &str,
    args: &serde_json::Value,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
) -> Result<String, String> {
    // 取出所需信息后立即释放 registry 锁（std::sync::MutexGuard 非 Send，不能跨 await 持有）
    let (entry_path, code, runtime) = {
        let registry = registry().lock().unwrap();
        let plugin = registry
            .get(plugin_name)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        let entry_path = plugin.dir.join(&plugin.manifest.entry);
        let code = std::fs::read_to_string(&entry_path).map_err(|e| format!("read entry: {e}"))?;
        (entry_path, code, plugin.manifest.runtime.clone())
    };

    match runtime.as_str() {
        "rhai" => exec_rhai(&code, args),
        "node" => exec_process("node", &entry_path, args, tool_name, api_key, api_base_url).await,
        "python" | "py" => {
            exec_process(
                "python",
                &entry_path,
                args,
                tool_name,
                api_key,
                api_base_url,
            )
            .await
        }
        "shell" | "bash" => {
            exec_process("bash", &entry_path, args, tool_name, api_key, api_base_url).await
        }
        other => Err(format!("unsupported runtime: {other}")),
    }
}

// ---- Rhai 脚本引擎 ----

fn exec_rhai(code: &str, _args: &serde_json::Value) -> Result<String, String> {
    // 安全沙箱：只允许字符串操作，直接返回脚本内容
    // 后续可集成真正的 rhai 引擎
    let result = code.trim().to_string();
    if result.is_empty() {
        Err("empty script".into())
    } else {
        Ok(result)
    }
}

// ---- 进程执行器 ----

/// 异步执行子进程（tokio::process），不阻塞 tokio 工作线程。
/// 参数通过 stdin 管道传入（JSON + 换行），脚本用 input()/stdin 读取。
/// tool_name 通过 RIPPLE_TOOL 环境变量传入，单脚本能分派多工具（不读则忽略，向后兼容）。
/// api_key/api_base_url 通过 RIPPLE_API_KEY/RIPPLE_API_BASE 传入（需凭证的工具用，如 image-gen）。
async fn exec_process(
    cmd: &str,
    script_path: &PathBuf,
    args: &serde_json::Value,
    tool_name: &str,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
) -> Result<String, String> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    const MAX_PLUGIN_INPUT_BYTES: usize = 256 * 1024;
    const MAX_PLUGIN_OUTPUT_BYTES: usize = 1024 * 1024;
    const PLUGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    let args_json = serde_json::to_string(args).map_err(|e| format!("serialize args: {e}"))?;
    if args_json.len() > MAX_PLUGIN_INPUT_BYTES {
        return Err(format!(
            "plugin input exceeds {} bytes",
            MAX_PLUGIN_INPUT_BYTES
        ));
    }
    let mut builder = tokio::process::Command::new(cmd);
    if let Some(plugin_dir) = script_path.parent() {
        builder.current_dir(plugin_dir);
    }
    builder
        .arg(script_path)
        .env("RIPPLE_TOOL", tool_name)
        // 强制 Python 用 UTF-8 stdio（Windows 默认 GBK，print 非 ASCII 字符会 UnicodeEncodeError，
        // 且后端 from_utf8_lossy 会把 GBK 字节当 UTF-8 解码成乱码）。node/shell 忽略这两个 env，无副作用。
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    // 凭证 env（仅 AI 工具调用链传入；execute_plugin_tool IPC 手动触发时为 None）
    if let Some(k) = api_key {
        builder.env("RIPPLE_API_KEY", k);
    }
    if let Some(u) = api_base_url {
        builder.env("RIPPLE_API_BASE", u);
    }
    builder.kill_on_drop(true);
    let mut child = builder
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("exec error: {e}"))?;

    // 通过 stdin 传入 JSON 参数（末尾换行让脚本的 input() 立即返回）
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(args_json.as_bytes())
            .await
            .map_err(|e| format!("write stdin: {e}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("write stdin: {e}"))?;
    } // stdin drop → 关闭管道 → 脚本 input() 收到 EOF

    let output = match tokio::time::timeout(PLUGIN_TIMEOUT, child.wait_with_output()).await {
        Ok(result) => result.map_err(|e| format!("wait: {e}"))?,
        Err(_) => {
            return Err(format!(
                "plugin timed out after {}s",
                PLUGIN_TIMEOUT.as_secs()
            ))
        }
    };

    let truncate = |bytes: &[u8]| {
        let truncated = bytes.len() > MAX_PLUGIN_OUTPUT_BYTES;
        let end = bytes.len().min(MAX_PLUGIN_OUTPUT_BYTES);
        let mut text = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
        if truncated {
            text.push_str("\n[output truncated]");
        }
        text
    };
    if output.status.success() {
        Ok(truncate(&output.stdout))
    } else {
        let stderr = truncate(&output.stderr);
        Err(format!(
            "exit {}: {stderr}",
            output.status.code().unwrap_or(-1)
        ))
    }
}

// ---- IPC 命令 ----

#[tauri::command]
pub async fn list_plugins() -> Result<Vec<PluginInfo>, String> {
    scan_plugins();
    let registry = registry().lock().unwrap();
    let mut plugins: Vec<_> = registry
        .values()
        .map(|p| PluginInfo {
            manifest: p.manifest.clone(),
            enabled: p.enabled,
        })
        .collect();
    plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(plugins)
}

#[tauri::command]
pub async fn toggle_plugin(name: String, enabled: bool) -> Result<(), String> {
    if !valid_plugin_name(&name) {
        return Err("invalid plugin name".into());
    }
    let mut state = read_enabled_state();
    let mut registry = registry().lock().unwrap();
    let plugin = registry
        .get_mut(&name)
        .ok_or_else(|| format!("plugin not found: {name}"))?;
    plugin.enabled = enabled;
    state.insert(name, enabled);
    if let Err(error) = write_enabled_state(&state) {
        plugin.enabled = !enabled;
        return Err(format!("persist plugin state: {error}"));
    }
    Ok(())
}

fn sensitive_fields(schema: Option<&serde_json::Value>) -> Vec<String> {
    schema
        .and_then(|v| v.get("properties"))
        .and_then(|v| v.as_object())
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(name, property)| {
                    let format = property
                        .get("format")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let sensitive = property
                        .get("sensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        || property
                            .get("writeOnly")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        || matches!(format, "password" | "secret")
                        || [
                            "password",
                            "secret",
                            "token",
                            "api_key",
                            "apikey",
                            "credential",
                        ]
                        .iter()
                        .any(|hint| name.to_ascii_lowercase().contains(hint));
                    sensitive.then(|| name.clone())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn plugin_schema(name: &str) -> Result<Option<serde_json::Value>, String> {
    let registry = registry().lock().unwrap();
    registry
        .get(name)
        .map(|p| p.manifest.config_schema.clone())
        .ok_or_else(|| format!("plugin not found: {name}"))
}

/// 读取插件配置。敏感字段只返回掩码，不把明文暴露给前端。
#[tauri::command]
pub async fn get_plugin_config(name: String) -> Result<serde_json::Value, String> {
    let dir = plugin_dir_for(&name)?;
    let schema = plugin_schema(&name)?;
    let config_path = dir.join("config.json");
    let mut config = match std::fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("read plugin config: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(format!("read plugin config: {e}")),
    };
    if let Some(object) = config.as_object_mut() {
        for field in sensitive_fields(schema.as_ref()) {
            if object.contains_key(&field) {
                object.insert(field, serde_json::Value::String(MASKED_VALUE.into()));
            }
        }
    }
    Ok(config)
}

/// 写入插件配置。掩码值表示保留现有敏感字段。
#[tauri::command]
pub async fn set_plugin_config(name: String, config: serde_json::Value) -> Result<(), String> {
    let dir = plugin_dir_for(&name)?;
    let schema = plugin_schema(&name)?;
    let mut config = config
        .as_object()
        .cloned()
        .ok_or("plugin config must be an object")?;
    let config_path = dir.join("config.json");
    let existing = match std::fs::read_to_string(&config_path) {
        Ok(content) => Some(
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| format!("read existing plugin config: {e}"))?,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("read existing plugin config: {e}")),
    };
    for field in sensitive_fields(schema.as_ref()) {
        if config.get(&field).and_then(|v| v.as_str()) == Some(MASKED_VALUE) {
            if let Some(value) = existing.as_ref().and_then(|v| v.get(&field)) {
                config.insert(field, value.clone());
            } else {
                config.remove(&field);
            }
        }
    }
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content).map_err(|e| format!("write plugin config: {e}"))
}

/// 手动 IPC 不允许直接启动宿主进程插件；这类工具必须通过聊天执行网关完成审批。
#[tauri::command]
pub async fn execute_plugin_tool(
    tool_name: String,
    args: serde_json::Value,
) -> Result<String, String> {
    let parts: Vec<&str> = tool_name.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err("invalid tool name, expected 'plugin.tool'".into());
    }
    let runtime = {
        let registry = registry().lock().unwrap();
        registry
            .get(parts[0])
            .ok_or_else(|| format!("plugin not found: {}", parts[0]))?
            .manifest
            .runtime
            .clone()
    };
    if runtime != "rhai" {
        return Err(
            "process plugins must be executed through the approved agent tool gateway".into(),
        );
    }
    exec_plugin_tool(parts[0], parts[1], &args, None, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_names_reject_path_traversal() {
        assert!(valid_plugin_name("file-ops_2"));
        for invalid in ["", ".", "..", "../evil", "a/b", "a\\b", "插件"] {
            assert!(!valid_plugin_name(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn entry_paths_must_stay_inside_plugin() {
        let base = std::path::Path::new("plugins/example");
        assert!(path_stays_within(base, "src/main.py"));
        assert!(!path_stays_within(base, "../secret.py"));
        assert!(!path_stays_within(base, "/tmp/script.py"));
        assert!(!path_stays_within(base, "src/../secret.py"));
    }

    #[test]
    fn sensitive_schema_fields_are_detected() {
        let schema = serde_json::json!({"properties": {
            "api_key": {"type": "string"},
            "custom": {"type": "string", "sensitive": true},
            "password": {"type": "string", "format": "password"},
            "model": {"type": "string"}
        }});
        let fields = sensitive_fields(Some(&schema));
        assert!(fields.contains(&"api_key".into()));
        assert!(fields.contains(&"custom".into()));
        assert!(fields.contains(&"password".into()));
        assert!(!fields.contains(&"model".into()));
    }
}

/// 回传工具审批结果（前端审批框调用）。唤醒对应 request_id 的 oneshot，让后端 await 继续。
/// trust_tool=true 表示用户勾选了「信任此工具」（仅 elevated 模式生效，由后端记录）。
#[tauri::command]
pub async fn approve_tool_call(
    request_id: String,
    approved: bool,
    trust_tool: bool,
) -> Result<(), String> {
    match take_pending_approval(&request_id) {
        Some(tx) => tx
            .send((approved, trust_tool))
            .map_err(|_| "approval receiver dropped".into()),
        None => Err("pending approval not found (已超时或已处理)".into()),
    }
}

// ---- Agent 权限管理 IPC ----

/// 读取 Agent 权限级别
#[tauri::command]
pub async fn get_agent_permission_level(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<String, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    Ok(agent_permission_level(&conn, &agent_id))
}

/// 设置 Agent 权限级别（strict / elevated / full）
#[tauri::command]
pub async fn set_agent_permission_level(
    state: State<'_, AppState>,
    agent_id: String,
    level: String,
) -> Result<(), String> {
    if !matches!(level.as_str(), "strict" | "elevated" | "full") {
        return Err(format!(
            "invalid permission level: {level} (expected strict/elevated/full)"
        ));
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE agents SET permission_level = ?1, updated_at = ?2 WHERE id = ?3",
        params![level, chrono::Utc::now().to_rfc3339(), agent_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 列出 Agent 已信任的工具
#[tauri::command]
pub async fn list_trusted_tools(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<String>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT tool_name FROM agent_trusted_tools WHERE agent_id = ?1 ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([agent_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

/// 收回某工具的信任
#[tauri::command]
pub async fn revoke_trust(
    state: State<'_, AppState>,
    agent_id: String,
    tool_name: String,
) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM agent_trusted_tools WHERE agent_id = ?1 AND tool_name = ?2",
        params![agent_id, tool_name],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
