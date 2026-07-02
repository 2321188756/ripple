//! 消息查询命令。

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
