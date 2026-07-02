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
