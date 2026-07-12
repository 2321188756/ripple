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
    /// 背景壁纸文件绝对路径（可选）。前端用 convertFileSrc 转成 webview 可访问 URL。
    #[serde(default)]
    pub wallpaper: Option<String>,
    /// 壁纸遮罩不透明度 0-100（0=全透壁纸全亮，100=全暗），默认 60。
    #[serde(default)]
    pub wallpaper_darkness: Option<f64>,
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
            wallpaper: None,
            wallpaper_darkness: None,
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
        "max_tokens": 3000,
        "temperature": 0.75,
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
    let msg = &json["choices"][0]["message"];
    // 优先 content；推理模型（deepseek/gemini）可能把内容放 reasoning_content 而 content 为空
    let reply = msg["content"].as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| msg["reasoning_content"].as_str())
        .ok_or_else(|| {
            let raw = serde_json::to_string(&json).unwrap_or_default();
            let head: String = raw.chars().take(400).collect();
            format!("LLM 未返回内容，原始响应: {head}")
        })?;
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
    // 读凭证 + 默认模型
    let (api_key, api_base_url, default_model) = {
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
    let model = model_override
        .filter(|s| !s.is_empty())
        .unwrap_or(default_model);

    let raw = call_llm_for_theme(&api_base_url, &api_key, &model, &prompt, image.as_deref()).await?;
    let json_str = extract_json_array(&raw);
    if json_str.trim().is_empty() {
        let head: String = raw.chars().take(400).collect();
        return Err(format!("LLM 返回空内容（可能 reasoning 耗尽 token 或模型异常），原始: {head}"));
    }
    let mut themes: Vec<ThemeDefinition> = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析主题 JSON 失败: {e}\n原始(前300字): {}", json_str.chars().take(300).collect::<String>()))?;

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
    tracing::info!(prompt = %prompt, model = %model, count = themes.len(), "AI themes generated");
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
