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

/// 列出 newapi 代理上所有可用模型（调 GET /v1/models）。
/// 用于模型选择器动态加载，取代硬编码列表。失败返回空数组（调用方 fallback 到内置列表）。
#[tauri::command]
pub async fn list_available_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let (api_key, api_base_url) = {
        let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
        let ak: String = conn.query_row("SELECT value FROM settings WHERE key='api_key'", [], |r| r.get(0)).unwrap_or_default();
        let au: String = conn.query_row("SELECT value FROM settings WHERE key='api_base_url'", [], |r| r.get(0)).unwrap_or_else(|_| "http://192.168.0.123:3000/v1".into());
        (ak, au)
    };
    if api_key.is_empty() {
        return Err("未配置 API Key".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .get(format!("{}/models", api_base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| format!("list models: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("list models http {status}: {b}"));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let models: Vec<String> = json["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    tracing::info!(count = models.len(), "fetched available models");
    Ok(models)
}
