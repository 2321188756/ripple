//! 消息查询与编辑命令。

use std::time::Duration;

use ripple_core::Message;
use ripple_conversation_store::{MessageRepo, SearchResult};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
    limit: Option<usize>,
    before_id: Option<String>,
) -> Result<Vec<Message>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    MessageRepo::list_by_conversation(
        &conn,
        &conversation_id,
        limit.unwrap_or(50),
        before_id.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    query: String,
    conversation_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    MessageRepo::search(&conn, &query, conversation_id.as_deref(), limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

/// 编辑消息内容。只允许编辑最后一条 user 消息。
/// 如果该消息之后还有 AI 回复，后续消息会被删除（保持对话连续）。
#[tauri::command]
pub async fn update_message(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> Result<Message, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    // 读取原消息
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, summary, created_at, token_count, metadata FROM messages WHERE id = ?1"
    ).map_err(|e| e.to_string())?;
    let msg = stmt.query_row([&id], |r| {
        let role_str: String = r.get(2)?;
        let content_json: String = r.get(3)?;
        Ok(Message {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            role: match role_str.as_str() {
                "system" => ripple_core::MessageRole::System,
                "user" => ripple_core::MessageRole::User,
                "assistant" => ripple_core::MessageRole::Assistant,
                "tool" => ripple_core::MessageRole::Tool,
                _ => ripple_core::MessageRole::User,
            },
            content: serde_json::from_str(&content_json).unwrap_or_default(),
            created_at: r.get::<_, String>(5).ok()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.to_utc())
                .unwrap_or_else(|| chrono::Utc::now()),
            token_count: r.get::<_, Option<i32>>(6).ok().flatten(),
            metadata: r.get::<_, String>(7).ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
        })
    }).map_err(|e| format!("message not found: {e}"))?;

    // 只允许编辑 user 消息
    if msg.role != ripple_core::MessageRole::User {
        return Err("Only user messages can be edited".into());
    }

    MessageRepo::update_content(&conn, &id, &content).map_err(|e| e.to_string())?;

    // 返回更新后的消息
    let mut updated = msg;
    updated.content = vec![ripple_core::ContentBlock::Text { text: content }];
    Ok(updated)
}

/// 删除指定消息及其之后的所有消息（保持对话连续性）
#[tauri::command]
pub async fn delete_messages_from(
    state: State<'_, AppState>,
    conversation_id: String,
    from_message_id: String,
) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    MessageRepo::delete_from(&conn, &conversation_id, &from_message_id)
        .map_err(|e| e.to_string())
}
