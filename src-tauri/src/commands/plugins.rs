//! 插件系统：扫描 plugins/ 目录，加载 manifest，注册工具，执行脚本。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use ripple_core::{ToolDefinition, ToolSource};

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

/// 将已加载插件的工具注册到工具列表
pub fn plugin_tools() -> Vec<ToolDefinition> {
    let registry = registry().lock().unwrap();
    let mut tools = Vec::new();
    for (name, plugin) in registry.iter() {
        if !plugin.enabled { continue; }
        for pt in &plugin.manifest.tools {
            tools.push(ToolDefinition {
                name: format!("plugin_{}:{}", name, pt.name),
                description: format!("[{}] {}", name, pt.description),
                parameters: pt.parameters.clone(),
                source: ToolSource::Plugin { plugin_id: name.clone() },
                requires_approval: plugin.manifest.runtime != "rhai",
            });
        }
    }
    tools
}

/// 执行插件工具
/// 从 "plugin_{name}:{tool_name}" 格式的完整工具名执行
pub fn exec_by_tool_name(full_name: &str, args: &serde_json::Value) -> Result<String, String> {
    // 工具注册名为 `plugin_{name}:{tool}`（见 plugin_tools），但注册表以 `{name}` 为键，
    // 故此处先剥离 `plugin_` 前缀再按 ':' 拆分，否则 registry.get 永远 NotFound。
    let stripped = full_name.strip_prefix("plugin_").unwrap_or(full_name);
    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("invalid plugin tool name: {full_name}"));
    }
    exec_plugin_tool(parts[0], parts[1], args)
}

pub fn exec_plugin_tool(plugin_name: &str, tool_name: &str, args: &serde_json::Value) -> Result<String, String> {
    let registry = registry().lock().unwrap();
    let plugin = registry.get(plugin_name).ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
    let entry_path = plugin.dir.join(&plugin.manifest.entry);
    let code = std::fs::read_to_string(&entry_path).map_err(|e| format!("read entry: {e}"))?;

    match plugin.manifest.runtime.as_str() {
        "rhai" => exec_rhai(&code, args),
        "node" => exec_process("node", &entry_path, args),
        "python" | "py" => exec_process("python", &entry_path, args),
        "shell" | "bash" => exec_process("bash", &entry_path, args),
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

fn exec_process(cmd: &str, script_path: &PathBuf, args: &serde_json::Value) -> Result<String, String> {
    let args_json = serde_json::to_string(args).unwrap_or_default();
    let output = std::process::Command::new(cmd)
        .arg(script_path)
        .arg(&args_json)
        .output()
        .map_err(|e| format!("exec error: {e}"))?;

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
    exec_plugin_tool(parts[0], parts[1], &args)
}
