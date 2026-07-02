//! 自定义 Agent 管理命令。

use std::time::Duration;
use serde::{Serialize, Deserialize};
use crate::state::AppState;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: String,
    pub model: String,
    pub icon: String,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<Agent>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, name, description, system_prompt, tools, model, icon, created_at, updated_at FROM agents ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Agent {
        id: r.get(0)?, name: r.get(1)?, description: r.get(2)?,
        system_prompt: r.get(3)?, tools: r.get(4)?, model: r.get(5)?,
        icon: r.get(6)?, created_at: r.get(7)?, updated_at: r.get(8)?,
    })).map_err(|e| e.to_string())?;
    let mut agents = Vec::new();
    for row in rows { agents.push(row.map_err(|e| e.to_string())?); }
    Ok(agents)
}

#[tauri::command]
pub async fn create_agent(state: State<'_, AppState>, name: String, description: Option<String>, system_prompt: Option<String>) -> Result<Agent, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO agents (id, name, description, system_prompt, tools, model, icon, created_at, updated_at) VALUES (?1,?2,?3,?4,'[]','','🤖',?5,?6)",
        rusqlite::params![id, name, description.unwrap_or_default(), system_prompt.unwrap_or_else(|| "You are a helpful assistant.".into()), now, now],
    ).map_err(|e| e.to_string())?;
    get_agent_by_id(&conn, &id)
}

#[tauri::command]
pub async fn update_agent(state: State<'_, AppState>, id: String, name: Option<String>, description: Option<String>, system_prompt: Option<String>, tools: Option<String>, model: Option<String>, icon: Option<String>) -> Result<Agent, String> {
    tracing::info!(%id, has_prompt = system_prompt.is_some(), "update_agent called");
    let now = chrono::Utc::now().to_rfc3339();
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    if let Some(ref v) = name { conn.execute("UPDATE agents SET name=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?; }
    if let Some(ref v) = description { conn.execute("UPDATE agents SET description=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?; }
    if let Some(ref v) = system_prompt {
        tracing::info!(prompt_len = v.len(), "updating system_prompt");
        conn.execute("UPDATE agents SET system_prompt=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?;
    }
    if let Some(ref v) = tools { conn.execute("UPDATE agents SET tools=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?; }
    if let Some(ref v) = model { conn.execute("UPDATE agents SET model=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?; }
    if let Some(ref v) = icon { conn.execute("UPDATE agents SET icon=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| e.to_string())?; }
    let result = get_agent_by_id(&conn, &id);
    tracing::info!("update_agent done");
    result
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM agents WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_agent(state: State<'_, AppState>, id: String) -> Result<Agent, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    get_agent_by_id(&conn, &id)
}

pub fn get_agent_by_id(conn: &rusqlite::Connection, id: &str) -> Result<Agent, String> {
    conn.query_row(
        "SELECT id, name, description, system_prompt, tools, model, icon, created_at, updated_at FROM agents WHERE id=?1", [id],
        |r| Ok(Agent {
            id: r.get(0)?, name: r.get(1)?, description: r.get(2)?,
            system_prompt: r.get(3)?, tools: r.get(4)?, model: r.get(5)?,
            icon: r.get(6)?, created_at: r.get(7)?, updated_at: r.get(8)?,
        })
    ).map_err(|e| e.to_string())
}
