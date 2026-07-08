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

/// 必需的 CSS 变量（缺失会导致界面不完整）。
const REQUIRED_VARS: &[&str] = &[
    "--background", "--foreground", "--primary", "--card", "--border",
    "--muted", "--sidebar-background",
];

/// 取某变量的默认 HSL 值（从 default-light 内置主题抽取）。
fn default_var_value(var: &str, dark: bool) -> &'static str {
    match (var, dark) {
        ("--background", false) => "240 10% 97%",
        ("--background", true) => "224 71% 4%",
        ("--foreground", false) => "224 71% 4%",
        ("--foreground", true) => "210 20% 92%",
        ("--primary", false) => "243 75% 59%",
        ("--primary", true) => "243 75% 67%",
        ("--card", false) => "0 0% 100%",
        ("--card", true) => "224 71% 8%",
        ("--border", false) => "220 13% 89%",
        ("--border", true) => "215 28% 18%",
        ("--muted", false) => "220 14% 95%",
        ("--muted", true) => "215 28% 14%",
        ("--sidebar-background", false) => "240 10% 97%",
        ("--sidebar-background", true) => "224 71% 5%",
        _ => "0 0% 50%",
    }
}

/// 校验并补全主题缺失的必需变量（降级逻辑：导入/AI 生成的主题若缺变量，自动补默认值）。
/// 返回补全的变量列表（供调用方记录日志/提示）。
pub fn ensure_required_vars(theme: &mut ThemeDefinition) -> Vec<String> {
    let mut filled = Vec::new();
    for var in REQUIRED_VARS {
        if !theme.colors.light.contains_key(*var) {
            theme.colors.light.insert((*var).to_string(), default_var_value(var, false).to_string());
            filled.push(format!("light {}", var));
        }
        if !theme.colors.dark.contains_key(*var) {
            theme.colors.dark.insert((*var).to_string(), default_var_value(var, true).to_string());
            filled.push(format!("dark {}", var));
        }
    }
    filled
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
    let mut theme: ThemeDefinition = serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))?;
    // 降级：补全缺失的必需变量
    let filled = ensure_required_vars(&mut theme);
    if !filled.is_empty() {
        tracing::info!(theme_id = %theme.id, filled = ?filled, "imported theme filled missing vars");
    }
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

/// 删除主题。内置主题不可删；正在使用的主题由前端拦截（activeThemeId 在 localStorage）。
#[tauri::command]
pub async fn delete_theme(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    if builtin_themes().iter().any(|t| t.id == id) {
        return Err("内置主题不可删除".into());
    }
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let mut themes = load_themes_from_db(&conn);
    let before = themes.len();
    themes.retain(|t| t.id != id);
    if themes.len() == before {
        return Err(format!("主题 {} 不存在", id));
    }
    save_themes_to_db(&conn, &themes)?;
    tracing::info!(theme_id = %id, "theme deleted");
    Ok(())
}

// ---- AI 主题生成 ----

/// 从 LLM 响应中提取 JSON 数组（剥离 markdown 代码块/解释文本）。
fn extract_json_array(raw: &str) -> String {
    let start = raw.find('[');
    let end = raw.rfind(']');
    match (start, end) {
        (Some(s), Some(e)) if s < e => raw[s..=e].to_string(),
        _ => raw.to_string(),
    }
}

/// 调 LLM 为关键词生成 3 套主题，返回原始响应文本。
async fn call_llm_for_theme(
    api_base_url: &str,
    api_key: &str,
    model: &str,
    keyword: &str,
) -> Result<String, String> {
    let prompt = r##"你是 UI 主题设计师。根据关键词「__KW__」为桌面 AI 助手应用生成 3 套完整主题。
返回纯 JSON 数组（不要 markdown 代码块、不要解释），每个元素结构：
{"id":"ai-x","name":"主题名（中文，2-4字）","description":"简短描述","isBuiltin":false,"colors":{"light":{"--background":"H S% L%","--foreground":"H S% L%","--primary":"H S% L%","--card":"H S% L%","--border":"H S% L%","--muted":"H S% L%","--sidebar-background":"H S% L%"},"dark":{"--background":"H S% L%","--foreground":"H S% L%","--primary":"H S% L%","--card":"H S% L%","--border":"H S% L%","--muted":"H S% L%","--sidebar-background":"H S% L%"}},"agentStyle":{"icon_color":"HEXCOLOR","border_color":"HEXCOLOR","border_width":2,"name_color":"HEXCOLOR"}}

规则：
1. 颜色用 HSL 格式 "H S% L%"（如 "210 20% 95%"），H 为 0-360，S/L 为百分比
2. 前景色与背景色对比度 ≥ 4.5:1（WCAG AA）
3. 暗色模式：背景 L ≤ 15%，前景 L ≥ 85%；浅色模式：背景 L ≥ 90%，前景 L ≤ 20%
4. primary 主色与 background 区分明显
5. 3 套主题风格要有差异（如明快/沉稳/对比）
6. agentStyle 的颜色用 #hex 格式（如 #6366f1），与主题协调
7. 只返回 JSON 数组，不要任何其他文字"##.replace("__KW__", keyword);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": 2500,
        "temperature": 0.85,
    });
    let resp = client
        .post(format!("{}/chat/completions", api_base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("llm call: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("llm http {status}: {b}"));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let reply = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("LLM 未返回内容")?;
    Ok(reply.to_string())
}

/// AI 生成主题：根据关键词调 LLM 生成 3 套候选主题，校验补全后返回。
#[tauri::command]
pub async fn generate_theme(
    state: State<'_, AppState>,
    keyword: String,
) -> Result<Vec<ThemeDefinition>, String> {
    if keyword.trim().is_empty() {
        return Err("关键词不能为空".into());
    }
    // 读凭证
    let (api_key, api_base_url, model) = {
        let conn = state.db.get_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())?;
        let ak: String = conn.query_row("SELECT value FROM settings WHERE key='api_key'", [], |r| r.get(0)).unwrap_or_default();
        let au: String = conn.query_row("SELECT value FROM settings WHERE key='api_base_url'", [], |r| r.get(0)).unwrap_or_else(|_| "http://192.168.0.123:3000/v1".into());
        let lm: String = conn.query_row("SELECT value FROM settings WHERE key='llm_model'", [], |r| r.get(0))
            .ok().filter(|s: &String| !s.is_empty()).unwrap_or_else(|| "deepseek-v4-flash".into());
        (ak, au, lm)
    };
    if api_key.is_empty() {
        return Err("未配置 API Key，请在设置中配置".into());
    }

    let raw = call_llm_for_theme(&api_base_url, &api_key, &model, &keyword).await?;
    let json_str = extract_json_array(&raw);
    let mut themes: Vec<ThemeDefinition> = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析主题 JSON 失败: {e}"))?;

    // 覆盖 id 确保唯一 + 校验补全
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    for (i, t) in themes.iter_mut().enumerate() {
        t.id = format!("ai-{}-{}", ts, i);
        t.is_builtin = Some(false);
        let filled = ensure_required_vars(t);
        if !filled.is_empty() {
            tracing::warn!(theme_name = %t.name, ?filled, "generated theme filled missing vars");
        }
    }
    if themes.is_empty() {
        return Err("LLM 未生成有效主题".into());
    }
    tracing::info!(keyword = %keyword, count = themes.len(), "AI themes generated");
    Ok(themes)
}
