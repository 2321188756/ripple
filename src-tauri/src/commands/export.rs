//! 对话导出/导入

use std::time::Duration;
use crate::state::AppState;
use tauri::State;
use ripple_core::{Message, MessageRole, ContentBlock};

#[tauri::command]
pub async fn export_conversation(state: State<'_, AppState>, id: String, format: Option<String>) -> Result<String, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    // 获取对话信息
    let conv = conn.query_row(
        "SELECT title, created_at FROM conversations WHERE id=?1", [&id],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?)),
    ).map_err(|e| format!("conversation not found: {e}"))?;

    // 获取消息
    let mut msgs: Vec<Message> = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, conversation_id, role, content, summary, created_at, token_count, metadata FROM messages WHERE conversation_id=?1 ORDER BY created_at ASC") {
        if let Ok(rows) = stmt.query_map([&id], |r| {
            let role_str: String = r.get(2)?;
            let content_json: String = r.get(3)?;
            Ok(Message {
                id: r.get(0)?, conversation_id: r.get(1)?,
                role: match role_str.as_str() { "system" => MessageRole::System, "user" => MessageRole::User, "assistant" => MessageRole::Assistant, "tool" => MessageRole::Tool, _ => MessageRole::User },
                content: serde_json::from_str(&content_json).unwrap_or_default(),
                created_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_,String>(5)?).map(|d| d.to_utc()).unwrap_or_else(|_| chrono::Utc::now()),
                token_count: r.get::<_,Option<i32>>(6).ok().flatten(),
                metadata: r.get::<_,String>(7).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            })
        }) {
            for row in rows.flatten() { msgs.push(row); }
        }
    }

    let fmt = format.unwrap_or_else(|| "markdown".into());

    if fmt == "json" {
        // JSON 格式
        let export = serde_json::json!({
            "title": conv.0,
            "created_at": conv.1,
            "messages": msgs.iter().map(|m| serde_json::json!({
                "role": match m.role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::Tool => "tool", _ => "user" },
                "content": m.text(),
                "created_at": m.created_at.to_rfc3339(),
            })).collect::<Vec<_>>(),
        });
        serde_json::to_string_pretty(&export).map_err(|e| e.to_string())
    } else {
        // Markdown 格式
        let mut md = format!("# {}\n\n> Exported on {}\n\n---\n\n", conv.0, chrono::Utc::now().format("%Y-%m-%d %H:%M"));
        for m in &msgs {
            let role = match m.role { MessageRole::User => "**You**", MessageRole::Assistant => "**AI**", MessageRole::System => "**System**", _ => "**Tool**" };
            md.push_str(&format!("{}\n\n{}\n\n---\n\n", role, m.text()));
        }
        Ok(md)
    }
}

#[tauri::command]
pub async fn import_conversation(state: State<'_, AppState>, json_data: String) -> Result<String, String> {
    let data: serde_json::Value = serde_json::from_str(&json_data).map_err(|e| format!("invalid json: {e}"))?;
    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Imported");
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    let conv_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at, model_id, provider_id, metadata) VALUES (?1,?2,?3,?4,'imported','imported','{}')",
        rusqlite::params![conv_id, title, now, now],
    ).map_err(|e| e.to_string())?;

    if let Some(messages) = data.get("messages").and_then(|v| v.as_array()) {
        for msg_data in messages {
            let role = msg_data.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg_data.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let created = msg_data.get("created_at").and_then(|v| v.as_str()).unwrap_or(&now);
            let msg_id = uuid::Uuid::new_v4().to_string();
            let content_json = serde_json::json!([{"type":"text","text":content}]).to_string();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![msg_id, conv_id, role, content_json, created],
            ).map_err(|e| e.to_string())?;
        }
    }

    Ok(conv_id)
}
