//! 日志管理命令：前端写入日志、读取日志。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// 全局日志文件路径（由 setup 时设置）
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_log_dir(path: PathBuf) {
    let _ = LOG_DIR.set(path);
}

pub fn get_log_dir() -> Option<&'static PathBuf> {
    LOG_DIR.get()
}

/// 前端主动写一条日志
#[tauri::command]
pub fn log_event(level: String, message: String) -> Result<(), String> {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        "info" => tracing::info!(target: "frontend", "{message}"),
        _ => tracing::debug!(target: "frontend", "{message}"),
    }
    Ok(())
}

/// 获取日志文件路径（前端可直接定位）
#[tauri::command]
pub fn get_log_path() -> Result<String, String> {
    let d = get_log_dir().ok_or("log dir not initialized")?;
    // 找最新的日志文件
    let mut latest: Option<std::path::PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(d) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()).map_or(false, |s| s.contains("ripple.log")) {
                match &latest {
                    None => latest = Some(path),
                    Some(existing) => {
                        let m1 = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                        let m2 = std::fs::metadata(existing).ok().and_then(|m| m.modified().ok());
                        if m1 > m2 { latest = Some(path); }
                    }
                }
            }
        }
    }
    latest.map(|p| p.to_string_lossy().to_string()).ok_or("no log file".into())
}

/// 读取日志文件末尾 N 行
#[tauri::command]
pub fn get_logs(lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_dir = get_log_dir().ok_or("log dir not initialized")?;
    // 找最新的 .log 文件
    let mut latest: Option<std::path::PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()).map_or(false, |s| s.contains("ripple.log")) {
                match &latest {
                    None => latest = Some(path),
                    Some(existing) => {
                        let m1 = std::fs::metadata(&path).ok().map(|m| m.modified().ok());
                        let m2 = std::fs::metadata(existing).ok().map(|m| m.modified().ok());
                        if m1 > m2 {
                            latest = Some(path);
                        }
                    }
                }
            }
        }
    }

    let log_path = latest.ok_or("no log file found")?;
    let content = std::fs::read_to_string(&log_path).map_err(|e| format!("read log: {e}"))?;

    let n = lines.unwrap_or(100);
    let all_lines: Vec<&str> = content.lines().collect();
    let start = if all_lines.len() > n { all_lines.len() - n } else { 0 };
    Ok(all_lines[start..].iter().map(|s| s.to_string()).collect())
}
