//! 设置读写命令（API Key / Base URL / 默认模型等）

use std::time::Duration;

use crate::state::AppState;
use ripple_security::EncryptedKey;
use rusqlite::OptionalExtension;
use tauri::State;

const DEFAULT_PROVIDER_ID: &str = "newapi";

pub fn load_api_key(state: &AppState) -> Result<String, String> {
    load_api_key_from_pool(&state.db, &state.key_manager)
}

pub fn load_api_key_from_pool(
    db: &ripple_conversation_store::DbPool,
    key_manager: &std::sync::Arc<ripple_security::KeyManager>,
) -> Result<String, String> {
    let conn = db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let encrypted = conn.query_row(
        "SELECT encrypted_key, nonce FROM api_keys WHERE provider_id = ?1",
        [DEFAULT_PROVIDER_ID],
        |row| {
            Ok(EncryptedKey {
                ciphertext: row.get(0)?,
                nonce: row.get(1)?,
            })
        },
    );
    match encrypted {
        Ok(value) => key_manager
            .decrypt(&value)
            .map_err(|_| "API key recovery failed".to_string()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err("API Key is not configured".into()),
        Err(error) => Err(error.to_string()),
    }
}

/// 将旧版 `settings.api_key` 原子迁移至加密的 `api_keys` 表。
/// 成功前始终保留旧值；失败由事务回滚，避免产生半迁移或丢失凭据。
pub fn migrate_legacy_api_key(
    db: &ripple_conversation_store::DbPool,
    key_manager: &std::sync::Arc<ripple_security::KeyManager>,
) -> Result<bool, String> {
    let mut conn = db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let legacy: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key='api_key'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(legacy) = legacy else {
        return Ok(false);
    };
    let api_key = legacy.trim();
    if api_key.is_empty() || api_key.len() > 8192 || api_key.contains(['\r', '\n']) {
        return Err("legacy API Key is invalid".into());
    }
    let encrypted = key_manager.encrypt(api_key).map_err(|e| e.to_string())?;
    if key_manager
        .decrypt(&encrypted)
        .map_err(|_| "legacy API Key encryption verification failed".to_string())?
        != api_key
    {
        return Err("legacy API Key encryption verification failed".into());
    }
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "INSERT OR IGNORE INTO provider_configs (id, display_name, provider_type, is_enabled, created_at, updated_at) VALUES (?1, ?1, 'custom_openai', 1, ?2, ?2)",
        rusqlite::params![DEFAULT_PROVIDER_ID, now],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO api_keys (provider_id, encrypted_key, nonce, created_at, updated_at) VALUES (?1,?2,?3,?4,?4) ON CONFLICT(provider_id) DO UPDATE SET encrypted_key=excluded.encrypted_key, nonce=excluded.nonce, updated_at=excluded.updated_at",
        rusqlite::params![DEFAULT_PROVIDER_ID, encrypted.ciphertext, encrypted.nonce, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM settings WHERE key='api_key'", [])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::info!("migrated legacy API key to encrypted storage");
    Ok(true)
}

#[tauri::command]
pub async fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    if key == "api_key" {
        return Err("API Key is write-only".into());
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let result = conn.query_row("SELECT value FROM settings WHERE key = ?1", [&key], |r| {
        r.get::<_, String>(0)
    });
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    if key == "api_key" {
        return Err("Use save_api_key for credentials".into());
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![key, value, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn save_api_key(state: State<'_, AppState>, api_key: String) -> Result<(), String> {
    let api_key = api_key.trim();
    if api_key.is_empty() || api_key.len() > 8192 || api_key.contains(['\r', '\n']) {
        return Err("invalid API Key".into());
    }
    let encrypted = state
        .key_manager
        .encrypt(api_key)
        .map_err(|e| e.to_string())?;
    let decrypted = state
        .key_manager
        .decrypt(&encrypted)
        .map_err(|e| e.to_string())?;
    if decrypted != api_key {
        return Err("API Key verification failed".into());
    }
    let mut conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO provider_configs (id, display_name, provider_type, is_enabled, created_at, updated_at) VALUES (?1, ?1, 'custom_openai', 1, ?2, ?2)",
        rusqlite::params![DEFAULT_PROVIDER_ID, chrono::Utc::now().to_rfc3339()],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO api_keys (provider_id, encrypted_key, nonce, created_at, updated_at) VALUES (?1,?2,?3,?4,?4) ON CONFLICT(provider_id) DO UPDATE SET encrypted_key=excluded.encrypted_key, nonce=excluded.nonce, updated_at=excluded.updated_at",
        rusqlite::params![DEFAULT_PROVIDER_ID, encrypted.ciphertext, encrypted.nonce, chrono::Utc::now().to_rfc3339()],
    ).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM settings WHERE key='api_key'", [])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    match load_api_key(&state) {
        Ok(_) => Ok(true),
        Err(error) if error == "API Key is not configured" => Ok(false),
        Err(error) => Err(error),
    }
}

#[tauri::command]
pub async fn clear_api_key(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM api_keys WHERE provider_id = ?1",
        [DEFAULT_PROVIDER_ID],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM settings WHERE key='api_key'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 切换 debug 日志级别。运行时生效（reload tracing filter）+ 持久化到 settings。
#[tauri::command]
pub async fn set_debug_logging(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    crate::set_debug_enabled(enabled);
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('debug_logging', ?1, ?2)",
        rusqlite::params![if enabled { "true" } else { "false" }, now],
    )
    .map_err(|e| e.to_string())?;
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
    let api_key = load_api_key(&state)?;
    let api_base_url: String = {
        let conn = state
            .db
            .get_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT value FROM settings WHERE key='api_base_url'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "http://192.168.0.123:3000/v1".into())
    };
    let resp = state
        .http_client
        .get(format!("{}/models", api_base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {api_key}"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("list models: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!(
            "list models provider request failed (HTTP {status})"
        ));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> (ripple_conversation_store::DbPool, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("ripple-settings-{}.db", uuid::Uuid::new_v4()));
        let pool = ripple_conversation_store::init_db(&path).unwrap();
        (pool, path)
    }

    #[test]
    fn migrates_legacy_api_key_atomically_to_encrypted_storage() {
        let (pool, path) = test_pool();
        let key_manager = std::sync::Arc::new(
            ripple_security::KeyManager::new("test-install-secret-0123456789", None).unwrap(),
        );
        let secret = "legacy-key-SENTINEL";
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('api_key', ?1, ?2)",
                rusqlite::params![secret, chrono::Utc::now().to_rfc3339()],
            )
            .unwrap();
        }

        assert!(migrate_legacy_api_key(&pool, &key_manager).unwrap());
        assert!(!migrate_legacy_api_key(&pool, &key_manager).unwrap());
        assert_eq!(load_api_key_from_pool(&pool, &key_manager).unwrap(), secret);

        let conn = pool.get().unwrap();
        let plaintext: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key='api_key'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert!(plaintext.is_none());
        let (ciphertext, nonce): (Vec<u8>, Vec<u8>) = conn
            .query_row(
                "SELECT encrypted_key, nonce FROM api_keys WHERE provider_id = ?1",
                [DEFAULT_PROVIDER_ID],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_ne!(ciphertext, secret.as_bytes());
        assert!(!nonce.is_empty());

        drop(conn);
        drop(pool);
        let _ = std::fs::remove_file(path);
    }
}
