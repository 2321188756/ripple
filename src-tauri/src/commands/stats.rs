//! 使用量统计

use std::time::Duration;
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct UsageStats {
    pub total_conversations: usize,
    pub total_messages: usize,
    pub total_tokens: usize,
    pub messages_by_role: Vec<RoleCount>,
    pub daily_stats: Vec<DailyStat>,
    pub top_models: Vec<ModelCount>,
}

#[derive(Serialize)]
pub struct RoleCount { pub role: String, pub count: usize }

#[derive(Serialize)]
pub struct DailyStat { pub date: String, pub messages: usize, pub tokens: usize }

#[derive(Serialize)]
pub struct ModelCount { pub model: String, pub conversations: usize }

#[tauri::command]
pub async fn get_usage_stats(state: State<'_, AppState>) -> Result<UsageStats, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    let total_conversations: usize = conn.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize;
    let total_messages: usize = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize;
    let total_tokens: usize = conn.query_row("SELECT COALESCE(SUM(token_count),0) FROM messages WHERE token_count IS NOT NULL", [], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize;

    // 按角色统计
    let mut messages_by_role = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT role, COUNT(*) FROM messages GROUP BY role ORDER BY COUNT(*) DESC") {
        if let Ok(rows) = stmt.query_map([], |r| Ok(RoleCount { role: r.get(0)?, count: r.get::<_, i64>(1)? as usize })) {
            for row in rows.flatten() { messages_by_role.push(row); }
        }
    }

    // 按天统计
    let mut daily_stats = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT substr(created_at,1,10) as d, COUNT(*), COALESCE(SUM(token_count),0) FROM messages GROUP BY d ORDER BY d DESC LIMIT 30") {
        if let Ok(rows) = stmt.query_map([], |r| Ok(DailyStat { date: r.get(0)?, messages: r.get::<_, i64>(1)? as usize, tokens: r.get::<_, i64>(2)? as usize })) {
            for row in rows.flatten() { daily_stats.push(row); }
        }
    }

    // 按模型统计
    let mut top_models = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT model_id, COUNT(*) FROM conversations GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 10") {
        if let Ok(rows) = stmt.query_map([], |r| Ok(ModelCount { model: r.get(0)?, conversations: r.get::<_, i64>(1)? as usize })) {
            for row in rows.flatten() { top_models.push(row); }
        }
    }

    Ok(UsageStats { total_conversations, total_messages, total_tokens, messages_by_role, daily_stats, top_models })
}
