//! 对话管理命令。

use std::time::Duration;

use chrono::Utc;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use ripple_core::Conversation;
use tauri::State;

use crate::state::AppState;

// ---- 供 chat 模块调用的内部函数（无 Tauri 装饰） ----

pub fn get_conversation_inner(
    conn: &PooledConnection<SqliteConnectionManager>,
    id: &str,
) -> Result<Conversation, String> {
    ripple_conversation_store::conversation_repo::ConversationRepo::get_by_id(conn, id)
        .map_err(|e| e.to_string())
}

// ---- Tauri 命令 ----

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    provider_id: Option<String>,
    model_id: Option<String>,
    title: Option<String>,
    system_prompt: Option<String>,
    agent_id: Option<String>,
) -> Result<Conversation, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    // 加载 agent 的 system_prompt
    let (sys, agent_id_val) = if let Some(ref aid) = agent_id {
        if let Ok(agent) = crate::commands::agents::get_agent_by_id(&conn, aid) {
            (Some(agent.system_prompt), Some(aid.clone()))
        } else { (system_prompt, None) }
    } else { (system_prompt, None) };

    let title = title.or_else(|| agent_id_val.as_ref().map(|_| "Agent Chat".into()));

    // 把 agent_id 存到 metadata
    let metadata = if let Some(ref a) = agent_id_val {
        serde_json::json!({"agent_id": a}).to_string()
    } else {
        "{}".into()
    };

    let now = Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let title_str = title.unwrap_or_else(|| "New Conversation".into());
    // 默认模型从 settings 读取（前端「默认模型」字段），空则回退 deepseek-v4-flash
    let default_model: String = conn
        .query_row("SELECT value FROM settings WHERE key='default_model'", [], |r| r.get::<_, String>(0))
        .ok().filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| "deepseek-v4-flash".into());
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at, model_id, provider_id, system_prompt, pinned, archived, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8)",
        rusqlite::params![id, title_str, now, now, model_id.as_deref().unwrap_or(&default_model), provider_id.as_deref().unwrap_or("newapi"), sys, metadata],
    ).map_err(|e| e.to_string())?;

    // 直接查返回完整对象
    let mut stmt = conn.prepare("SELECT id, title, created_at, updated_at, model_id, provider_id, system_prompt, pinned, archived, metadata FROM conversations WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row([&id], |r| {
        Ok(Conversation {
            id: r.get(0)?, title: r.get(1)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(2)?).map(|d| d.to_utc()).unwrap_or_else(|_| Utc::now()),
            updated_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(3)?).map(|d| d.to_utc()).unwrap_or_else(|_| Utc::now()),
            model_id: r.get(4)?, provider_id: r.get(5)?, system_prompt: r.get(6)?,
            pinned: r.get::<_, i32>(7)? != 0, archived: r.get::<_, i32>(8)? != 0,
            metadata: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
        })
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_conversations(
    state: State<'_, AppState>,
    search: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    agent_id: Option<String>,
) -> Result<Vec<Conversation>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    // 按 agent_id 过滤
    if let Some(ref aid) = agent_id {
        let filter = format!("%\"agent_id\":\"{}\"%", aid.replace('"', "\\\""));
        let sql = "SELECT id, title, created_at, updated_at, model_id, provider_id, system_prompt, pinned, archived, metadata FROM conversations WHERE metadata LIKE ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3";
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![filter, limit.unwrap_or(50) as i64, offset.unwrap_or(0) as i64], map_row)
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows.flatten() { out.push(row); }
        return Ok(out);
    }

    ripple_conversation_store::conversation_repo::ConversationRepo::list(
        &conn,
        search.as_deref(),
        limit.unwrap_or(50),
        offset.unwrap_or(0),
    )
    .map_err(|e| e.to_string())
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: r.get(0)?, title: r.get(1)?,
        created_at: r.get::<_, String>(2).ok().and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.to_utc()).unwrap_or_else(|| Utc::now()),
        updated_at: r.get::<_, String>(3).ok().and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.to_utc()).unwrap_or_else(|| Utc::now()),
        model_id: r.get(4)?, provider_id: r.get(5)?, system_prompt: r.get(6)?,
        pinned: r.get::<_, i32>(7)? != 0, archived: r.get::<_, i32>(8)? != 0,
        metadata: r.get::<_, String>(9).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn get_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<Conversation, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    get_conversation_inner(&conn, &id)
}

#[tauri::command]
pub async fn update_conversation(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    system_prompt: Option<String>,
    pinned: Option<bool>,
    archived: Option<bool>,
) -> Result<Conversation, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    ripple_conversation_store::conversation_repo::ConversationRepo::update(
        &conn,
        &id,
        title.as_deref(),
        system_prompt.as_deref(),
        pinned,
        archived,
        None,
        None,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    ripple_conversation_store::conversation_repo::ConversationRepo::delete(&conn, &id)
        .map_err(|e| e.to_string())
}
