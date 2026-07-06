//! 设置读写命令（API Key / Base URL / 默认模型等）

use std::time::Duration;

use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1", [&key],
        |r| r.get::<_, String>(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![key, value, now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// 切换 debug 日志级别。运行时生效（reload tracing filter）+ 持久化到 settings。
#[tauri::command]
pub async fn set_debug_logging(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    crate::set_debug_enabled(enabled);
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('debug_logging', ?1, ?2)",
        rusqlite::params![if enabled { "true" } else { "false" }, now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取当前 debug 日志开关状态。
#[tauri::command]
pub async fn get_debug_logging(_state: State<'_, AppState>) -> Result<bool, String> {
    Ok(crate::debug_enabled())
}
