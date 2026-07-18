//! 主题管理命令：存储/读取/导入/导出

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub light: std::collections::HashMap<String, String>,
    pub dark: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThemeStyle {
    #[serde(alias = "icon_color")]
    pub icon_color: Option<String>,
    #[serde(alias = "border_color")]
    pub border_color: Option<String>,
    #[serde(alias = "border_width")]
    pub border_width: Option<f64>,
    #[serde(alias = "name_color")]
    pub name_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(alias = "is_builtin")]
    pub is_builtin: Option<bool>,
    pub colors: ThemeColors,
    #[serde(alias = "agent_style")]
    pub agent_style: Option<AgentThemeStyle>,
    /// 背景壁纸文件绝对路径（可选）。前端用 convertFileSrc 转成 webview 可访问 URL。
    #[serde(default)]
    pub wallpaper: Option<String>,
    /// 壁纸遮罩不透明度 0-100（0=全透壁纸全亮，100=全暗），默认 60。
    #[serde(default, alias = "wallpaper_darkness")]
    pub wallpaper_darkness: Option<f64>,
}

/// 内建主题
fn builtin_themes() -> Vec<ThemeDefinition> {
    let mut themes = vec![
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
                ]
                .into(),
                dark: [
                    ("--background".into(), "224 71% 4%".into()),
                    ("--foreground".into(), "210 20% 92%".into()),
                    ("--primary".into(), "243 75% 67%".into()),
                    ("--card".into(), "224 71% 8%".into()),
                    ("--border".into(), "215 28% 18%".into()),
                    ("--muted".into(), "215 28% 14%".into()),
                    ("--sidebar-background".into(), "224 71% 5%".into()),
                ]
                .into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#6366f1".into()),
                border_color: Some("#6366f1".into()),
                border_width: Some(2.0),
                name_color: Some("#1e293b".into()),
            }),
            wallpaper: None,
            wallpaper_darkness: None,
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
                ]
                .into(),
                dark: [
                    ("--background".into(), "222 47% 11%".into()),
                    ("--foreground".into(), "210 40% 98%".into()),
                    ("--primary".into(), "217 91% 60%".into()),
                    ("--card".into(), "222 47% 14%".into()),
                    ("--border".into(), "217 33% 20%".into()),
                    ("--sidebar-background".into(), "222 47% 11%".into()),
                ]
                .into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#3b82f6".into()),
                border_color: Some("#3b82f6".into()),
                border_width: Some(2.0),
                name_color: Some("#e2e8f0".into()),
            }),
            wallpaper: None,
            wallpaper_darkness: None,
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
                ]
                .into(),
                dark: [
                    ("--background".into(), "150 30% 8%".into()),
                    ("--foreground".into(), "140 15% 90%".into()),
                    ("--primary".into(), "142 70% 50%".into()),
                    ("--card".into(), "150 30% 12%".into()),
                    ("--border".into(), "140 15% 20%".into()),
                ]
                .into(),
            },
            agent_style: Some(AgentThemeStyle {
                icon_color: Some("#22c55e".into()),
                border_color: Some("#22c55e".into()),
                border_width: Some(2.0),
                name_color: Some("#166534".into()),
            }),
            wallpaper: None,
            wallpaper_darkness: None,
        },
    ];
    for theme in &mut themes {
        ensure_required_vars(theme);
    }
    themes
}

fn load_themes_from_db(conn: &rusqlite::Connection) -> Vec<ThemeDefinition> {
    let json_str: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key='themes'", [], |r| {
            r.get(0)
        })
        .ok();
    let mut themes: Vec<ThemeDefinition> = json_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    for theme in &mut themes {
        ensure_required_vars(theme);
    }
    // 合并内置主题
    let builtins = builtin_themes();
    for b in &builtins {
        if !themes.iter().any(|t| t.id == b.id) {
            themes.push(b.clone());
        }
    }
    themes
}

fn save_themes_to_db(
    conn: &rusqlite::Connection,
    themes: &[ThemeDefinition],
) -> Result<(), String> {
    let json_str = serde_json::to_string(themes).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('themes', ?1, ?2)",
        rusqlite::params![json_str, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Complete CSS palette contract. Missing values are filled for legacy imports and AI output,
/// while unknown user-defined variables are always retained.
const REQUIRED_VARS: &[&str] = &[
    "--background",
    "--foreground",
    "--card",
    "--card-foreground",
    "--popover",
    "--popover-foreground",
    "--primary",
    "--primary-foreground",
    "--secondary",
    "--secondary-foreground",
    "--muted",
    "--muted-foreground",
    "--accent",
    "--accent-foreground",
    "--destructive",
    "--destructive-foreground",
    "--warning",
    "--warning-foreground",
    "--success",
    "--success-foreground",
    "--info",
    "--info-foreground",
    "--border",
    "--input",
    "--ring",
    "--primary-50",
    "--primary-100",
    "--primary-200",
    "--primary-300",
    "--primary-400",
    "--primary-500",
    "--primary-600",
    "--primary-700",
    "--primary-800",
    "--primary-900",
    "--sidebar-background",
    "--sidebar-foreground",
    "--sidebar-primary",
    "--sidebar-primary-foreground",
    "--sidebar-accent",
    "--sidebar-accent-foreground",
    "--sidebar-border",
    "--sidebar-ring",
    "--gradient-from",
    "--gradient-via",
    "--gradient-to",
];

const LIGHT_DEFAULTS: &[(&str, &str)] = &[
    ("--background", "240 10% 97%"),
    ("--foreground", "224 71% 4%"),
    ("--card", "0 0% 100%"),
    ("--card-foreground", "224 71% 4%"),
    ("--popover", "0 0% 100%"),
    ("--popover-foreground", "224 71% 4%"),
    ("--primary", "243 75% 59%"),
    ("--primary-foreground", "0 0% 100%"),
    ("--secondary", "220 14% 96%"),
    ("--secondary-foreground", "220 9% 20%"),
    ("--muted", "220 14% 95%"),
    ("--muted-foreground", "220 9% 46%"),
    ("--accent", "220 14% 94%"),
    ("--accent-foreground", "220 9% 20%"),
    ("--destructive", "0 84% 56%"),
    ("--destructive-foreground", "0 0% 100%"),
    ("--warning", "38 92% 50%"),
    ("--warning-foreground", "0 0% 100%"),
    ("--success", "152 65% 45%"),
    ("--success-foreground", "0 0% 100%"),
    ("--info", "199 89% 48%"),
    ("--info-foreground", "0 0% 100%"),
    ("--border", "220 13% 89%"),
    ("--input", "220 13% 89%"),
    ("--ring", "243 75% 59%"),
    ("--sidebar-background", "240 10% 97%"),
    ("--sidebar-foreground", "220 9% 30%"),
    ("--sidebar-primary", "243 75% 59%"),
    ("--sidebar-primary-foreground", "0 0% 100%"),
    ("--sidebar-accent", "220 14% 93%"),
    ("--sidebar-accent-foreground", "220 9% 20%"),
    ("--sidebar-border", "220 13% 89%"),
    ("--sidebar-ring", "243 75% 59%"),
    ("--gradient-from", "243 75% 59%"),
    ("--gradient-via", "277 70% 62%"),
    ("--gradient-to", "199 89% 48%"),
];

const DARK_DEFAULTS: &[(&str, &str)] = &[
    ("--background", "224 71% 4%"),
    ("--foreground", "210 20% 92%"),
    ("--card", "224 71% 8%"),
    ("--card-foreground", "210 20% 92%"),
    ("--popover", "224 71% 6%"),
    ("--popover-foreground", "210 20% 92%"),
    ("--primary", "243 75% 67%"),
    ("--primary-foreground", "0 0% 100%"),
    ("--secondary", "215 28% 15%"),
    ("--secondary-foreground", "210 20% 92%"),
    ("--muted", "215 28% 14%"),
    ("--muted-foreground", "217 15% 62%"),
    ("--accent", "215 28% 16%"),
    ("--accent-foreground", "210 20% 92%"),
    ("--destructive", "0 72% 50%"),
    ("--destructive-foreground", "0 0% 100%"),
    ("--warning", "38 92% 55%"),
    ("--warning-foreground", "0 0% 100%"),
    ("--success", "152 60% 50%"),
    ("--success-foreground", "0 0% 100%"),
    ("--info", "199 89% 55%"),
    ("--info-foreground", "0 0% 100%"),
    ("--border", "215 28% 18%"),
    ("--input", "215 28% 18%"),
    ("--ring", "243 75% 67%"),
    ("--sidebar-background", "224 71% 5%"),
    ("--sidebar-foreground", "210 20% 92%"),
    ("--sidebar-primary", "243 75% 67%"),
    ("--sidebar-primary-foreground", "0 0% 100%"),
    ("--sidebar-accent", "215 28% 15%"),
    ("--sidebar-accent-foreground", "210 20% 92%"),
    ("--sidebar-border", "215 28% 18%"),
    ("--sidebar-ring", "243 75% 67%"),
    ("--gradient-from", "243 75% 67%"),
    ("--gradient-via", "277 70% 70%"),
    ("--gradient-to", "199 89% 60%"),
];

fn defaults_for_mode(dark: bool) -> &'static [(&'static str, &'static str)] {
    if dark {
        DARK_DEFAULTS
    } else {
        LIGHT_DEFAULTS
    }
}

fn palette_default(var: &str, dark: bool) -> &'static str {
    defaults_for_mode(dark)
        .iter()
        .find(|(key, _)| *key == var)
        .map(|(_, value)| *value)
        .unwrap_or("0 0% 50%")
}

fn primary_ramp_value(primary: &str, lightness: u8) -> Option<String> {
    let parts: Vec<&str> = primary.split_whitespace().collect();
    if parts.len() != 3 || !parts[1].ends_with('%') || !parts[2].ends_with('%') {
        return None;
    }
    let hue = parts[0].parse::<f64>().ok()?;
    let saturation = parts[1].trim_end_matches('%').parse::<f64>().ok()?;
    let _lightness = parts[2].trim_end_matches('%').parse::<f64>().ok()?;
    if !(0.0..=100.0).contains(&saturation) {
        return None;
    }
    Some(format!(
        "{} {}% {}%",
        hue.rem_euclid(360.0),
        saturation,
        lightness
    ))
}

fn apply_derived_aliases(palette: &mut std::collections::HashMap<String, String>, dark: bool) {
    let Some(primary) = palette.get("--primary").cloned() else {
        return;
    };
    for (token, lightness) in [
        ("--primary-50", 97),
        ("--primary-100", 93),
        ("--primary-200", 86),
        ("--primary-300", 76),
        ("--primary-400", if dark { 72 } else { 66 }),
        ("--primary-500", if dark { 67 } else { 59 }),
        ("--primary-600", if dark { 60 } else { 51 }),
        ("--primary-700", if dark { 52 } else { 43 }),
        ("--primary-800", 34),
        ("--primary-900", 25),
    ] {
        if !palette.contains_key(token) {
            if let Some(value) = primary_ramp_value(&primary, lightness) {
                palette.insert(token.to_string(), value);
            }
        }
    }
    for token in [
        "--ring",
        "--sidebar-primary",
        "--sidebar-ring",
        "--gradient-from",
    ] {
        palette
            .entry(token.to_string())
            .or_insert_with(|| primary.clone());
    }
    palette
        .entry("--primary-foreground".to_string())
        .or_insert_with(|| "0 0% 100%".to_string());
    palette
        .entry("--sidebar-primary-foreground".to_string())
        .or_insert_with(|| "0 0% 100%".to_string());
}

/// Complete missing palette entries while retaining arbitrary user-defined variables.
/// Legacy themes inherit brand aliases first, avoiding fallback indigo ramps.
pub fn ensure_required_vars(theme: &mut ThemeDefinition) -> Vec<String> {
    let mut filled = Vec::new();
    for (dark, mode) in [(false, "light"), (true, "dark")] {
        let palette = if dark {
            &mut theme.colors.dark
        } else {
            &mut theme.colors.light
        };
        apply_derived_aliases(palette, dark);
        for var in REQUIRED_VARS {
            if !palette.contains_key(*var) {
                palette.insert((*var).to_string(), palette_default(var, dark).to_string());
                filled.push(format!("{} {}", mode, var));
            }
        }
    }
    filled
}

#[tauri::command]
pub async fn list_themes(state: State<'_, AppState>) -> Result<Vec<ThemeDefinition>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    Ok(load_themes_from_db(&conn))
}

#[tauri::command]
pub async fn save_themes(
    state: State<'_, AppState>,
    themes: Vec<ThemeDefinition>,
) -> Result<(), String> {
    let mut normalized = themes;
    for theme in &mut normalized {
        let filled = ensure_required_vars(theme);
        if !filled.is_empty() {
            tracing::info!(theme_id = %theme.id, filled = ?filled, "saved theme filled missing vars");
        }
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    save_themes_to_db(&conn, &normalized)
}

#[tauri::command]
pub async fn export_theme(
    state: State<'_, AppState>,
    id: String,
    file_path: String,
) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let themes = load_themes_from_db(&conn);
    let theme = themes
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "Theme not found".to_string())?;
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
    let mut theme: ThemeDefinition =
        serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))?;
    // 降级：补全缺失的必需变量
    let filled = ensure_required_vars(&mut theme);
    if !filled.is_empty() {
        tracing::info!(theme_id = %theme.id, filled = ?filled, "imported theme filled missing vars");
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
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
pub async fn delete_theme(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if builtin_themes().iter().any(|t| t.id == id) {
        return Err("内置主题不可删除".into());
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
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
    prompt: &str,
    image: Option<&str>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    // 有图片时用多模态消息（vision 模型分析图片配色），否则纯文本
    let content: serde_json::Value = match image {
        Some(img) => serde_json::json!([
            { "type": "text", "text": prompt },
            { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{}", img) } }
        ]),
        None => serde_json::json!(prompt),
    };
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": content }],
        "max_tokens": 9000,
        "temperature": 0.75,
    });
    let resp = client
        .post(format!(
            "{}/chat/completions",
            api_base_url.trim_end_matches('/')
        ))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("llm call: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!(
            "theme generation provider request failed (HTTP {status})"
        ));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let msg = &json["choices"][0]["message"];
    // 优先 content；推理模型（deepseek/gemini）可能把内容放 reasoning_content 而 content 为空
    let reply = msg["content"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| msg["reasoning_content"].as_str())
        .ok_or_else(|| "theme generation returned no usable content".to_string())?;
    tracing::debug!(reply_len = reply.len(), "LLM theme reply");
    Ok(reply.to_string())
}

/// AI 生成主题：根据需求描述调 LLM 生成 3 套候选主题，校验补全后返回。
/// model_override 非空则用指定模型，否则用 settings 的 llm_model。
#[tauri::command]
pub async fn generate_theme(
    state: State<'_, AppState>,
    prompt: String,
    model_override: Option<String>,
    image: Option<String>,
) -> Result<Vec<ThemeDefinition>, String> {
    if prompt.trim().is_empty() {
        return Err("需求描述不能为空".into());
    }
    let api_key = crate::commands::settings::load_api_key(&state)?;
    let (api_base_url, default_model) = {
        let conn = state
            .db
            .get_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        let au: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key='api_base_url'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "http://192.168.0.123:3000/v1".into());
        let lm: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key='llm_model'",
                [],
                |r| r.get(0),
            )
            .ok()
            .filter(|s: &String| !s.is_empty())
            .unwrap_or_else(|| "deepseek-v4-flash".into());
        (au, lm)
    };
    let model = model_override
        .filter(|s| !s.is_empty())
        .unwrap_or(default_model);

    let raw =
        call_llm_for_theme(&api_base_url, &api_key, &model, &prompt, image.as_deref()).await?;
    let json_str = extract_json_array(&raw);
    if json_str.trim().is_empty() {
        return Err("theme generation returned empty content".into());
    }
    let mut themes: Vec<ThemeDefinition> = serde_json::from_str(&json_str)
        .map_err(|_| "theme generation returned invalid theme data".to_string())?;

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
    tracing::info!(
        model = %model,
        prompt_chars = prompt.chars().count(),
        has_image = image.is_some(),
        count = themes.len(),
        "AI themes generated"
    );
    Ok(themes)
}

/// 壁纸目录：用户主目录下的 ripple_wallpapers/
fn wallpaper_dir() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    home.join("ripple_wallpapers")
}

/// 保存壁纸：把用户选择的图片复制到 ~/ripple_wallpapers/，返回目标绝对路径。
/// 主题的 wallpaper 字段存这个路径，前端用 convertFileSrc 显示。
#[tauri::command]
pub async fn save_wallpaper(src_path: String, theme_id: String) -> Result<String, String> {
    let dir = wallpaper_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create wallpaper dir: {e}"))?;
    let src = std::path::Path::new(&src_path);
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("png");
    // 文件名：theme_id + 时间戳，避免冲突
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let dest = dir.join(format!("{}_{}.{}", theme_id, ts, ext));
    std::fs::copy(src, &dest).map_err(|e| format!("copy wallpaper: {e}"))?;
    let path = dest.to_string_lossy().to_string();
    tracing::info!(%path, "wallpaper saved");
    Ok(path)
}

/// 读取壁纸文件并返回 base64 data URL（供前端直接设 backgroundImage，避免 asset protocol / fs 权限问题）。
#[tauri::command]
pub async fn read_wallpaper_base64(path: String) -> Result<String, String> {
    use base64::Engine as _;
    let data = std::fs::read(&path).map_err(|e| format!("read wallpaper: {e}"))?;
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("jpg")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_theme(primary: &str) -> ThemeDefinition {
        ThemeDefinition {
            id: "legacy".into(),
            name: "Legacy".into(),
            description: None,
            is_builtin: Some(false),
            colors: ThemeColors {
                light: [
                    ("--background".into(), "120 20% 97%".into()),
                    ("--foreground".into(), "150 30% 10%".into()),
                    ("--primary".into(), primary.into()),
                    ("--card".into(), "0 0% 100%".into()),
                    ("--border".into(), "140 15% 85%".into()),
                    ("--muted".into(), "140 10% 93%".into()),
                    ("--sidebar-background".into(), "120 20% 97%".into()),
                ]
                .into(),
                dark: [
                    ("--background".into(), "150 30% 8%".into()),
                    ("--foreground".into(), "140 15% 90%".into()),
                    ("--primary".into(), primary.into()),
                    ("--card".into(), "150 30% 12%".into()),
                    ("--border".into(), "140 15% 20%".into()),
                    ("--muted".into(), "150 18% 16%".into()),
                    ("--sidebar-background".into(), "150 30% 8%".into()),
                ]
                .into(),
            },
            agent_style: None,
            wallpaper: None,
            wallpaper_darkness: None,
        }
    }

    #[test]
    fn completion_preserves_unknown_tokens_and_derives_primary_ramp() {
        let mut theme = legacy_theme("142 70% 45%");
        theme
            .colors
            .light
            .insert("--brand-glow".into(), "310 90% 60%".into());

        ensure_required_vars(&mut theme);

        assert_eq!(theme.colors.light["--brand-glow"], "310 90% 60%");
        assert_eq!(theme.colors.light["--ring"], "142 70% 45%");
        assert_eq!(theme.colors.light["--gradient-from"], "142 70% 45%");
        assert_eq!(theme.colors.light["--primary-500"], "142 70% 59%");
        assert_eq!(theme.colors.dark["--primary-500"], "142 70% 67%");
        for token in REQUIRED_VARS {
            assert!(theme.colors.light.contains_key(*token));
            assert!(theme.colors.dark.contains_key(*token));
        }
    }

    #[test]
    fn camel_case_agent_style_and_builtin_deserialize_with_snake_case_compatibility() {
        let camel_case = r##"{
            "id":"generated","name":"Generated","isBuiltin":false,
            "agentStyle":{"iconColor":"#22c55e","borderColor":"#166534","borderWidth":2,"nameColor":"#14532d"},
            "colors":{"light":{"--primary":"142 70% 45%"},"dark":{"--primary":"142 70% 50%"}}
        }"##;
        let snake_case = r##"{
            "id":"legacy","name":"Legacy","is_builtin":false,
            "agent_style":{"icon_color":"#22c55e","border_color":"#166534","border_width":2,"name_color":"#14532d"},
            "colors":{"light":{"--primary":"142 70% 45%"},"dark":{"--primary":"142 70% 50%"}}
        }"##;

        for json in [camel_case, snake_case] {
            let theme: ThemeDefinition =
                serde_json::from_str(json).expect("theme should deserialize");
            let style = theme.agent_style.expect("agent style should deserialize");
            assert_eq!(theme.is_builtin, Some(false));
            assert_eq!(style.icon_color.as_deref(), Some("#22c55e"));
            assert_eq!(style.border_width, Some(2.0));
        }
    }

    #[test]
    fn builtins_are_returned_with_complete_palettes() {
        for theme in builtin_themes() {
            for token in REQUIRED_VARS {
                assert!(theme.colors.light.contains_key(*token));
                assert!(theme.colors.dark.contains_key(*token));
            }
        }
    }
}
