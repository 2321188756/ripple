//! 自定义 Agent 管理命令。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
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
    // Agent 样式
    pub icon_color: String,
    pub border_color: String,
    pub border_width: f64,
    pub name_color: String,
    // 模型参数
    pub temperature: f64,
    pub max_tokens: u32,
    pub top_p: f64,
}

const COLUMNS: &str =
    "id, name, description, system_prompt, tools, model, icon, created_at, updated_at, \
    icon_color, border_color, border_width, name_color, temperature, max_tokens, top_p";

fn row_to_agent(r: &rusqlite::Row) -> rusqlite::Result<Agent> {
    Ok(Agent {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        system_prompt: r.get(3)?,
        tools: r.get(4)?,
        model: r.get(5)?,
        icon: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        icon_color: r.get(9)?,
        border_color: r.get(10)?,
        border_width: r.get(11)?,
        name_color: r.get(12)?,
        temperature: r.get(13)?,
        max_tokens: r.get(14)?,
        top_p: r.get(15)?,
    })
}

pub fn get_agent_by_id(conn: &rusqlite::Connection, id: &str) -> Result<Agent, String> {
    let sql = format!("SELECT {COLUMNS} FROM agents WHERE id=?1");
    conn.query_row(&sql, [id], row_to_agent)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<Agent>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let sql = format!("SELECT {COLUMNS} FROM agents ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_agent)
        .map_err(|e| e.to_string())?;
    let mut agents = Vec::new();
    for row in rows {
        agents.push(row.map_err(|e| e.to_string())?);
    }
    Ok(agents)
}

#[tauri::command]
pub async fn create_agent(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    system_prompt: Option<String>,
) -> Result<Agent, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO agents (id, name, description, system_prompt, tools, model, icon, created_at, updated_at, \
         icon_color, border_color, border_width, name_color, temperature, max_tokens, top_p) \
         VALUES (?1,?2,?3,?4,'[]','','🤖',?5,?6,'#6366f1','#6366f1',2,'#1e293b',0.7,4096,1.0)",
        rusqlite::params![id, name, description.unwrap_or_default(), system_prompt.unwrap_or_else(|| "You are a helpful assistant.".into()), now, now],
    ).map_err(|e| e.to_string())?;
    get_agent_by_id(&conn, &id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_agent(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    system_prompt: Option<String>,
    tools: Option<String>,
    model: Option<String>,
    icon: Option<String>,
    icon_color: Option<String>,
    border_color: Option<String>,
    border_width: Option<f64>,
    name_color: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
) -> Result<Agent, String> {
    tracing::info!(%id, "update_agent called");
    let now = chrono::Utc::now().to_rfc3339();
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let mut updated = false;
    macro_rules! set {
        ($val:expr, $col:expr) => {
            if let Some(ref v) = $val {
                conn.execute(
                    &format!("UPDATE agents SET {}=?1, updated_at=?2 WHERE id=?3", $col),
                    rusqlite::params![v, now, id],
                )
                .map_err(|e| e.to_string())?;
                updated = true;
            }
        };
    }
    set!(name, "name");
    set!(description, "description");
    set!(system_prompt, "system_prompt");
    set!(tools, "tools");
    set!(model, "model");
    set!(icon, "icon");
    set!(icon_color, "icon_color");
    set!(border_color, "border_color");
    set!(border_width, "border_width");
    set!(name_color, "name_color");
    set!(temperature, "temperature");
    set!(max_tokens, "max_tokens");
    set!(top_p, "top_p");
    if !updated {
        return get_agent_by_id(&conn, &id);
    }
    get_agent_by_id(&conn, &id)
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM agents WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_agent(state: State<'_, AppState>, id: String) -> Result<Agent, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    get_agent_by_id(&conn, &id)
}
