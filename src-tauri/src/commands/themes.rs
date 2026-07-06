//! 主题管理命令：存储/读取/导入/导出

use std::time::Duration;
use serde::{Serialize, Deserialize};
use crate::state::AppState;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub light: std::collections::HashMap<String, String>,
    pub dark: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThemeStyle {
    pub icon_color: Option<String>,
    pub border_color: Option<String>,
    pub border_width: Option<f64>,
    pub name_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: Option<bool>,
    pub colors: ThemeColors,
    pub agent_style: Option<AgentThemeStyle>,
}

/// 内建主题
fn builtin_themes() -> Vec<ThemeDefinition> {
    vec![
        ThemeDefinition {
            id: "default-light".into(),
            name: "默认浅色".into(),
            description: Some("Ripple 默认浅色主题".into()),
            is_builtin: Some(true),
            colors: ThemeColors {
                light: [
                    ("--background".into(), "240 10% 97%".into()),
                    ("--foreground".into(), "224 71% 4%".into()),
                    ("--primary".into(), "243 75% 59%".into()),
                    ("--card".into(), "0 0% 100%".into()),
                    ("--border".into(), "220 13% 89%".into()),
                    ("--muted".into(), "220 14% 95%".into()),
                    ("--sidebar-background".into(), "240 10% 97%".into()),
                ].into(),
                dark: [
                    ("--background".into(), "224 71% 4%".into()),
                    ("--foreground".into(), "210 20% 92%".into()),
                    ("--primary".into(), "243 75% 67%".into()),
                    ("--card".into(), "224 71% 8%".into()),
                    ("--border".into(), "215 28% 18%".into()),
                    ("--muted".into(), "215 28% 14%".into()),
                    ("--sidebar-background".into(), "224 71% 5%".into()),
                ].into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#6366f1".into()),
                border_color: Some("#6366f1".into()),
                border_width: Some(2.0),
                name_color: Some("#1e293b".into()),
            }),
        },
        ThemeDefinition {
            id: "midnight".into(),
            name: "午夜".into(),
            description: Some("深邃暗色主题".into()),
            is_builtin: Some(true),
            colors: ThemeColors {
                light: [
                    ("--background".into(), "222 47% 11%".into()),
                    ("--foreground".into(), "210 40% 98%".into()),
                    ("--primary".into(), "217 91% 60%".into()),
                ].into(),
                dark: [
                    ("--background".into(), "222 47% 11%".into()),
                    ("--foreground".into(), "210 40% 98%".into()),
                    ("--primary".into(), "217 91% 60%".into()),
                    ("--card".into(), "222 47% 14%".into()),
                    ("--border".into(), "217 33% 20%".into()),
                    ("--sidebar-background".into(), "222 47% 11%".into()),
                ].into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#3b82f6".into()),
                border_color: Some("#3b82f6".into()),
                border_width: Some(2.0),
                name_color: Some("#e2e8f0".into()),
            }),
        },
        ThemeDefinition {
            id: "nature".into(),
            name: "自然".into(),
            description: Some("绿色护眼主题".into()),
            is_builtin: Some(true),
            colors: ThemeColors {
                light: [
                    ("--background".into(), "120 20% 97%".into()),
                    ("--foreground".into(), "150 30% 10%".into()),
                    ("--primary".into(), "142 70% 45%".into()),
                    ("--card".into(), "0 0% 100%".into()),
                    ("--border".into(), "140 15% 85%".into()),
                ].into(),
                dark: [
                    ("--background".into(), "150 30% 8%".into()),
                    ("--foreground".into(), "140 15% 90%".into()),
                    ("--primary".into(), "142 70% 50%".into()),
                    ("--card".into(), "150 30% 12%".into()),
                    ("--border".into(), "140 15% 20%".into()),
                ].into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#22c55e".into()),
                border_color: Some("#22c55e".into()),
                border_width: Some(2.0),
                name_color: Some("#166534".into()),
            }),
        },
    ]
}

fn load_themes_from_db(conn: &rusqlite::Connection) -> Vec<ThemeDefinition> {
    let json_str: Option<String> = conn.query_row(
        "SELECT value FROM settings WHERE key='themes'", [], |r| r.get(0),
    ).ok();
    let mut themes: Vec<ThemeDefinition> = json_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // 合并内置主题
    let builtins = builtin_themes();
    for b in &builtins {
        if !themes.iter().any(|t| t.id == b.id) {
            themes.push(b.clone());
        }
    }
    themes
}

fn save_themes_to_db(conn: &rusqlite::Connection, themes: &[ThemeDefinition]) -> Result<(), String> {
    let json_str = serde_json::to_string(themes).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('themes', ?1, ?2)",
        rusqlite::params![json_str, now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_themes(state: State<'_, AppState>) -> Result<Vec<ThemeDefinition>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    Ok(load_themes_from_db(&conn))
}

#[tauri::command]
pub async fn save_themes(
    state: State<'_, AppState>,
    themes: Vec<ThemeDefinition>,
) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    save_themes_to_db(&conn, &themes)
}

#[tauri::command]
pub async fn export_theme(
    state: State<'_, AppState>,
    id: String,
    file_path: String,
) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let themes = load_themes_from_db(&conn);
    let theme = themes.into_iter().find(|t| t.id == id).ok_or_else(|| "Theme not found".to_string())?;
    let json = serde_json::to_string_pretty(&theme).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, json).map_err(|e| format!("write file: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn import_theme(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<ThemeDefinition, String> {
    let json = std::fs::read_to_string(&file_path).map_err(|e| format!("read file: {e}"))?;
    let theme: ThemeDefinition = serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))?;
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let mut themes = load_themes_from_db(&conn);
    // 覆盖或追加
    if let Some(pos) = themes.iter().position(|t| t.id == theme.id) {
        themes[pos] = theme.clone();
    } else {
        themes.push(theme.clone());
    }
    save_themes_to_db(&conn, &themes)?;
    Ok(theme)
}
