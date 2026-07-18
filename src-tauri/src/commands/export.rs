//! 对话导出/导入

use std::time::Duration;

use ripple_core::{ContentBlock, MessageRole};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

const EXPORT_VERSION: u32 = 1;
const MAX_IMPORT_BYTES: usize = 10 * 1024 * 1024;
const MAX_IMPORT_MESSAGES: usize = 10_000;

#[derive(Serialize, Deserialize)]
struct ConversationExportV1 {
    version: u32,
    conversation: ExportedConversation,
    messages: Vec<ExportedMessage>,
}

#[derive(Serialize, Deserialize)]
struct ExportedConversation {
    title: String,
    created_at: String,
    model_id: String,
    provider_id: String,
    system_prompt: Option<String>,
    metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct ExportedMessage {
    role: MessageRole,
    content: Vec<ContentBlock>,
    created_at: String,
    token_count: Option<i32>,
    metadata: serde_json::Value,
}

#[tauri::command]
pub async fn export_conversation(
    state: State<'_, AppState>,
    id: String,
    format: Option<String>,
) -> Result<String, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let conversation = conn.query_row(
        "SELECT title, created_at, model_id, provider_id, system_prompt, metadata FROM conversations WHERE id=?1",
        [&id],
        |row| Ok(ExportedConversation {
            title: row.get(0)?,
            created_at: row.get(1)?,
            model_id: row.get(2)?,
            provider_id: row.get(3)?,
            system_prompt: row.get(4)?,
            metadata: row.get::<_, String>(5).ok().and_then(|value| serde_json::from_str(&value).ok()).unwrap_or_default(),
        }),
    ).map_err(|e| format!("conversation not found: {e}"))?;
    let messages =
        ripple_conversation_store::MessageRepo::list_by_conversation(&conn, &id, usize::MAX, None)
            .map_err(|e| e.to_string())?;

    if format.as_deref() == Some("json") {
        let export = ConversationExportV1 {
            version: EXPORT_VERSION,
            conversation,
            messages: messages
                .into_iter()
                .map(|message| ExportedMessage {
                    role: message.role,
                    content: message.content,
                    created_at: message.created_at.to_rfc3339(),
                    token_count: message.token_count,
                    metadata: message.metadata,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&export).map_err(|e| e.to_string())
    } else {
        let mut markdown = format!(
            "# {}\n\n> Exported on {}\n\n---\n\n",
            conversation.title,
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        );
        for message in messages {
            let role = match message.role {
                MessageRole::User => "**You**",
                MessageRole::Assistant => "**AI**",
                MessageRole::System => "**System**",
                MessageRole::Tool => "**Tool**",
            };
            markdown.push_str(&format!("{role}\n\n{}\n\n---\n\n", message.text()));
        }
        Ok(markdown)
    }
}

#[tauri::command]
pub async fn import_conversation(
    state: State<'_, AppState>,
    json_data: String,
) -> Result<String, String> {
    if json_data.len() > MAX_IMPORT_BYTES {
        return Err(format!("import exceeds {MAX_IMPORT_BYTES} bytes"));
    }
    let import: ConversationExportV1 =
        serde_json::from_str(&json_data).map_err(|e| format!("invalid export: {e}"))?;
    if import.version != EXPORT_VERSION {
        return Err(format!("unsupported export version: {}", import.version));
    }
    if import.conversation.title.trim().is_empty() {
        return Err("conversation title cannot be empty".into());
    }
    if import.messages.len() > MAX_IMPORT_MESSAGES {
        return Err(format!(
            "import contains too many messages: {}",
            import.messages.len()
        ));
    }
    let created_at = chrono::DateTime::parse_from_rfc3339(&import.conversation.created_at)
        .map_err(|e| format!("invalid conversation timestamp: {e}"))?
        .to_utc();
    let messages = import
        .messages
        .into_iter()
        .map(|message| {
            let timestamp = chrono::DateTime::parse_from_rfc3339(&message.created_at)
                .map_err(|e| format!("invalid message timestamp: {e}"))?
                .to_utc();
            if message.content.is_empty() {
                return Err("message content cannot be empty".to_string());
            }
            Ok((message, timestamp))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let conversation_id = uuid::Uuid::new_v4().to_string();
    let updated_at = messages
        .iter()
        .map(|(_, timestamp)| *timestamp)
        .max()
        .unwrap_or(created_at);
    tx.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at, model_id, provider_id, system_prompt, metadata) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            conversation_id,
            import.conversation.title,
            created_at.to_rfc3339(),
            updated_at.to_rfc3339(),
            import.conversation.model_id,
            import.conversation.provider_id,
            import.conversation.system_prompt,
            import.conversation.metadata.to_string(),
        ],
    ).map_err(|e| e.to_string())?;
    for (message, timestamp) in messages {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        tx.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, metadata) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                conversation_id,
                role,
                serde_json::to_string(&message.content).map_err(|e| e.to_string())?,
                timestamp.to_rfc3339(),
                message.token_count,
                message.metadata.to_string(),
            ],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(conversation_id)
}
