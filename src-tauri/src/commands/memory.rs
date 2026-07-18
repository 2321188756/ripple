//! Agent 记忆系统：文件扫描 → hash 检测 → 分块嵌入 → 向量存储。
//!
//! 记忆原文件在 dailynote/{agentName}/ 目录（用户可编辑），
//! 系统根据文件 SHA256 变更增量重建 memories 表索引。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use ripple_conversation_store::{MemoryChunk, MemoryFileMeta, MemoryRepo};
use ripple_rag::{chunk_text, read_file_content, ChunkConfig, EmbeddingClient};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::state::AppState;

// ---- 路径工具 ----

fn project_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") {
                d.pop();
                d.pop();
            }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
                d.pop();
            }
            d
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn dailynote_dir() -> PathBuf {
    project_root().join("dailynote")
}

fn validate_memory_path(root: &Path, file_path: &str, must_exist: bool) -> Result<PathBuf, String> {
    let relative = Path::new(file_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("memory path must be a normalized relative path".into());
    }
    if !matches!(
        relative.extension().and_then(|ext| ext.to_str()),
        Some("md") | Some("txt")
    ) {
        return Err("memory path must reference a .md or .txt file".into());
    }

    let root = root
        .canonicalize()
        .map_err(|e| format!("resolve memory root: {e}"))?;
    let candidate = root.join(relative);
    let resolved = if must_exist {
        candidate
            .canonicalize()
            .map_err(|e| format!("resolve memory file: {e}"))?
    } else {
        let parent = candidate.parent().ok_or("memory path has no parent")?;
        let parent = parent
            .canonicalize()
            .map_err(|e| format!("resolve memory parent: {e}"))?;
        parent.join(
            candidate
                .file_name()
                .ok_or("memory path has no file name")?,
        )
    };
    if !resolved.starts_with(&root) {
        return Err("memory path escapes the agent memory directory".into());
    }
    Ok(resolved)
}

fn agent_name_by_id(
    db: &ripple_conversation_store::DbPool,
    agent_id: &str,
) -> Result<String, String> {
    let conn = db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.query_row("SELECT name FROM agents WHERE id = ?1", [agent_id], |row| {
        row.get(0)
    })
    .map_err(|e| format!("agent not found: {e}"))
}

fn resolve_agent_memory_file(
    db: &ripple_conversation_store::DbPool,
    agent_id: &str,
    file_path: &str,
    must_exist: bool,
) -> Result<PathBuf, String> {
    let agent_name = agent_name_by_id(db, agent_id)?;
    let agent_root = ensure_agent_memory_dir(&agent_name)?;
    let expected_prefix = format!("dailynote/{}/", sanitize_dir_name(&agent_name));
    let relative = file_path.replace('\\', "/");
    let relative = relative
        .strip_prefix(&expected_prefix)
        .ok_or("memory path does not belong to the requested agent")?;
    validate_memory_path(&agent_root, relative, must_exist)
}

fn active_memory_operations() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

struct MemoryOperationGuard {
    agent_id: String,
}

impl MemoryOperationGuard {
    fn acquire(agent_id: &str) -> Result<Self, String> {
        let mut active = active_memory_operations()
            .lock()
            .map_err(|_| "memory operation lock poisoned")?;
        if !active.insert(agent_id.to_string()) {
            return Err("该 Agent 的记忆正在处理中，请稍后再试".into());
        }
        Ok(Self {
            agent_id: agent_id.to_string(),
        })
    }
}

impl Drop for MemoryOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = active_memory_operations().lock() {
            active.remove(&self.agent_id);
        }
    }
}

fn sanitize_dir_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 某 Agent 的记忆目录：dailynote/{sanitized_name}/
pub fn agent_memory_dir(agent_name: &str) -> PathBuf {
    dailynote_dir().join(sanitize_dir_name(agent_name))
}

/// 确保 Agent 的记忆目录存在（dailynote/{name}/）。
/// 不创建初始文件——remember 工具写入时间戳命名的 .txt 文件。
pub fn ensure_agent_memory_dir(agent_name: &str) -> Result<PathBuf, String> {
    let dir = agent_memory_dir(agent_name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create memory dir: {e}"))?;
    Ok(dir)
}

// ---- 关键词提取（替代 embedding 的标签检索）----

/// 停用词表（中英文高频无意义词，用于关键词提取）
const STOP_WORDS: &[&str] = &[
    "的", "了", "是", "在", "有", "和", "就", "不", "人", "都", "一", "个", "上", "也", "很", "到",
    "说", "要", "去", "你", "会", "着", "没", "有", "看", "好", "自", "己", "这", "他", "她", "它",
    "们", "我", "那", "对", "与", "为", "之", "及", "但", "可", "被", "从", "而", "或", "如", "因",
    "所", "其", "中", "外", "后", "前", "能", "做", "用", "让", "问", "是", "吗", "吧", "啊", "呢",
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "can", "could", "shall", "should", "may", "might",
    "must", "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "my",
    "your", "his", "her", "its", "our", "their", "me", "him", "us", "them", "to", "in", "of",
    "for", "on", "with", "at", "by", "from", "as", "into", "through", "and", "or", "but", "not",
    "if", "so", "than", "between", "about", "after", "before",
];

/// 解析文件中的 tag 行。格式：`Tag: 关键词1, 关键词2, 关键词3`
/// 兼容旧格式：`#tags：美食，汉堡，吃饭`、`#[tag:weight, tag:weight]`。
pub fn parse_tag_line(content: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return (content.to_string(), None);
    }
    let last = lines[lines.len() - 1].trim();
    // VCP 格式：`Tag: 关键词1, 关键词2, 关键词3`
    if let Some(tag_str) = last
        .strip_prefix("Tag: ")
        .or_else(|| last.strip_prefix("tag: "))
    {
        let tags: Vec<(String, usize)> = tag_str
            .split(',')
            .filter_map(|t| {
                let t = t.trim().to_lowercase();
                if t.is_empty() {
                    None
                } else {
                    Some((t, 1))
                }
            })
            .collect();
        if !tags.is_empty() {
            let json = serde_json::to_string(
                &tags
                    .iter()
                    .map(|(t, w)| serde_json::json!({"t": t, "w": w}))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
            let tag_idx = content
                .rfind("Tag:")
                .or_else(|| content.rfind("tag:"))
                .or_else(|| content.rfind('#'))
                .unwrap_or(content.len());
            let clean = content[..tag_idx].trim_end().to_string();
            return (clean, Some(json));
        }
    }
    // 旧格式兼容：`#tags：美食，汉堡，吃饭`
    if let Some(tag_str) = last.strip_prefix("#tags：") {
        let tags: Vec<(String, usize)> = tag_str
            .split('，')
            .filter_map(|t| {
                let t = t.trim();
                if t.is_empty() {
                    None
                } else {
                    Some((t.to_lowercase(), 1))
                }
            })
            .collect();
        if !tags.is_empty() {
            let json = serde_json::to_string(
                &tags
                    .iter()
                    .map(|(t, w)| serde_json::json!({"t": t, "w": w}))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
            let clean = content[..content.rfind('#').unwrap_or(content.len())]
                .trim_end()
                .to_string();
            return (clean, Some(json));
        }
    }
    // 旧格式兼容：`#[tag:weight, tag:weight]`
    if last.starts_with("#[") && last.ends_with(']') {
        let inner = &last[2..last.len() - 1];
        let tags: Vec<(String, usize)> = inner
            .split(',')
            .filter_map(|part| {
                let parts: Vec<&str> = part.trim().splitn(2, ':').collect();
                if parts.len() == 2 {
                    let tag = parts[0].trim().to_lowercase();
                    let w: usize = parts[1].trim().parse().unwrap_or(1);
                    Some((tag, w))
                } else {
                    None
                }
            })
            .collect();
        if !tags.is_empty() {
            let json = serde_json::to_string(
                &tags
                    .iter()
                    .map(|(t, w)| serde_json::json!({"t": t, "w": w}))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
            let clean = content[..content.rfind('#').unwrap_or(content.len())]
                .trim_end()
                .to_string();
            return (clean, Some(json));
        }
    }
    (content.to_string(), None)
}

/// 提取 3-5 个语义化关键词（含权重）。
/// 策略：对中文用 2-gram/3-gram 组合，对英文用原词，过滤数字/日期/时间/停用词。
pub fn extract_keywords_weighted(text: &str) -> Vec<(String, usize)> {
    use std::collections::{HashMap, HashSet};

    // 停用字：n-gram 中含任一字则整组丢弃
    let stop_chars: HashSet<char> = [
        '的', '了', '是', '在', '有', '和', '就', '不', '人', '都', '一', '个', '上', '也', '很',
        '到', '说', '要', '去', '你', '会', '着', '没', '看', '好', '自', '己', '这', '他', '她',
        '它', '们', '我', '那', '对', '与', '为', '之', '及', '但', '可', '被', '从', '而', '或',
        '如', '因', '所', '其', '中', '外', '后', '前', '能', '做', '用', '让', '为', '该', '什',
        '么', '怎', '几', '哪', '谁', '何', '凡', '均', '各',
    ]
    .iter()
    .copied()
    .collect();

    // 分离连续中文块和英文词
    let mut cjk_blocks: Vec<String> = Vec::new();
    let mut eng_tokens: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut in_cjk = false;

    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            if in_cjk && !buf.is_empty() {
                cjk_blocks.push(buf.clone().to_lowercase());
                buf.clear();
            }
            buf.push(c);
            in_cjk = false;
        } else if c.is_alphanumeric() {
            if !in_cjk && !buf.is_empty() {
                eng_tokens.push(buf.clone().to_lowercase());
                buf.clear();
            }
            buf.push(c);
            in_cjk = true;
        } else {
            if !buf.is_empty() {
                if in_cjk {
                    cjk_blocks.push(buf.clone().to_lowercase());
                } else {
                    eng_tokens.push(buf.clone().to_lowercase());
                }
                buf.clear();
            }
        }
    }
    if !buf.is_empty() {
        if in_cjk {
            cjk_blocks.push(buf.to_lowercase());
        } else {
            eng_tokens.push(buf.to_lowercase());
        }
    }

    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut added: HashSet<String> = HashSet::new();

    // 从连续中文块提取 2-gram / 3-gram
    for block in &cjk_blocks {
        let chars: Vec<char> = block.chars().collect();
        for i in 0..chars.len() {
            for len in 2..=3 {
                if i + len > chars.len() {
                    break;
                }
                let gram: String = chars[i..i + len].iter().collect();
                // 关键过滤：gram 中任意字符是停用字、数字 → 丢弃
                if gram
                    .chars()
                    .any(|c| stop_chars.contains(&c) || c.is_ascii_digit())
                {
                    continue;
                }
                if added.insert(gram.clone()) {
                    *freq.entry(gram).or_default() += 1;
                }
            }
        }
    }

    // 英文词
    for t in &eng_tokens {
        if t.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if t.len() <= 1 || STOP_WORDS.contains(&t.as_str()) {
            continue;
        }
        if added.insert(t.clone()) {
            *freq.entry(t.clone()).or_default() += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.len().cmp(&a.0.len())));
    sorted.into_iter().take(5).collect()
}

// ---- 文件 hash ----

fn file_hash(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read for hash: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// 相对路径（存 DB 用，如 dailynote/Aemeath/notes.md）
fn rel_path(file: &Path) -> String {
    file.strip_prefix(project_root())
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---- 扫描目录 ----

fn scan_memory_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "md" || ext == "txt" {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

// ---- API 凭据（从 settings 读）----

async fn get_api_creds(
    db: &ripple_conversation_store::DbPool,
    key_manager: &std::sync::Arc<ripple_security::KeyManager>,
) -> Result<(String, String), String> {
    let api_key = crate::commands::settings::load_api_key_from_pool(db, key_manager)?;
    let api_base_url = {
        let conn = db
            .get_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT value FROM settings WHERE key='api_base_url'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "http://192.168.0.123:3000/v1".into())
    };
    Ok((api_key, api_base_url))
}

// ---- 核心索引引擎 ----

/// 扫描 Agent 的记忆目录，增量重建索引（hash 变更的文件重新分块嵌入）。
/// 返回本次重建的文件数。
pub async fn scan_and_index_with_db(
    db: &ripple_conversation_store::DbPool,
    key_manager: &std::sync::Arc<ripple_security::KeyManager>,
    agent_id: &str,
) -> Result<usize, String> {
    // 1. 查 agent name
    let agent_name = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let name: String = conn
            .query_row("SELECT name FROM agents WHERE id = ?1", [agent_id], |r| {
                r.get(0)
            })
            .map_err(|e| format!("agent not found: {e}"))?;
        name
    };

    let dir = ensure_agent_memory_dir(&agent_name)?;
    let files = scan_memory_files(&dir)?;

    // 2. DB 中已有的文件列表
    let existing: Vec<MemoryFileMeta> = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        MemoryRepo::list_files_by_agent(&conn, agent_id).map_err(|e| e.to_string())?
    };

    // 3. 获取 API 凭据 + 构建 embedding client
    let (api_key, api_base_url) = get_api_creds(db, key_manager).await?;
    let client = EmbeddingClient::new(&api_base_url, &api_key, "Qwen/Qwen3-Embedding-8B")?;

    let mut indexed = 0;
    let mut current_paths: Vec<String> = Vec::new();

    for file in &files {
        let rel = rel_path(file);
        current_paths.push(rel.clone());

        let hash = file_hash(file)?;
        let stored = {
            let conn = db
                .get_timeout(Duration::from_secs(3))
                .map_err(|e| e.to_string())?;
            MemoryRepo::get_file_hash(&conn, agent_id, &rel).map_err(|e| e.to_string())?
        };
        if stored.as_deref() == Some(&hash) {
            continue;
        }

        let content = read_file_content(&file.to_string_lossy())?;
        // 检查文件是否有 tag 行（#tag:weight, ...），有则解析并移除（不纳入 embedding）
        let (clean_content, tag_line) = parse_tag_line(&content);
        let chunks = chunk_text(&clean_content, &rel, agent_id, &ChunkConfig::default());
        if chunks.is_empty() {
            continue;
        }

        let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = match tokio::time::timeout(
            Duration::from_secs(30),
            client.embed_batch(&texts),
        )
        .await
        {
            Ok(Ok(e)) => e,
            Ok(Err(e)) => {
                tracing::warn!(error = %e, file = %rel, "memory embed failed, storing without embedding");
                Vec::new()
            }
            Err(_) => {
                tracing::warn!(file = %rel, "memory embed timeout 30s, storing without embedding");
                Vec::new()
            }
        };

        let chunk_data: Vec<(String, Option<String>, String)> = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let emb = embeddings
                    .get(i)
                    .map(|e| serde_json::to_string(e).unwrap_or_default());
                // 优先使用文件的 tag_line（若有）；否则提取关键词
                let tags_json = if let Some(ref tl) = tag_line {
                    tl.clone()
                } else {
                    let weighted = extract_keywords_weighted(&c.content);
                    serde_json::to_string(
                        &weighted
                            .iter()
                            .map(|(t, w)| serde_json::json!({"t": t, "w": w}))
                            .collect::<Vec<_>>(),
                    )
                    .unwrap_or_default()
                };
                (c.content.clone(), emb, tags_json)
            })
            .collect();

        {
            let conn = db
                .get_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            MemoryRepo::replace_file_chunks(&conn, agent_id, &rel, &hash, &chunk_data)
                .map_err(|e| e.to_string())?;
        }
        tracing::info!(file = %rel, chunks = chunk_data.len(), "memory indexed");
        indexed += 1;
    }

    // 4. 清理已删除的文件
    for meta in &existing {
        if !current_paths.contains(&meta.file_path) {
            let conn = db
                .get_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            MemoryRepo::delete_by_file(&conn, agent_id, &meta.file_path)
                .map_err(|e| e.to_string())?;
            tracing::info!(file = %meta.file_path, "memory file removed, cleaned index");
        }
    }

    Ok(indexed)
}

/// 启动时后台索引所有 Agent 的记忆
pub async fn index_all_agents(
    db: ripple_conversation_store::DbPool,
    key_manager: std::sync::Arc<ripple_security::KeyManager>,
) {
    let agent_ids: Vec<String> = match db.get_timeout(Duration::from_secs(5)) {
        Ok(conn) => conn
            .prepare("SELECT id FROM agents")
            .ok()
            .map(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, String>(0))
                    .ok()
                    .map(|rows| rows.flatten().collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        Err(_) => return,
    };
    for agent_id in agent_ids {
        match scan_and_index_with_db(&db, &key_manager, &agent_id).await {
            Ok(n) => tracing::info!(agent_id = %agent_id, indexed = n, "startup memory index done"),
            Err(e) => {
                tracing::warn!(agent_id = %agent_id, error = %e, "startup memory index failed")
            }
        }
    }
}

// ---- IPC 命令 ----

/// 重建指定 Agent 的记忆索引
#[tauri::command]
pub async fn reindex_memories(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<usize, String> {
    tracing::info!(%agent_id, "reindex_memories");
    let _guard = MemoryOperationGuard::acquire(&agent_id)?;
    scan_and_index_with_db(&state.db, &state.key_manager, &agent_id).await
}

/// 列出 Agent 的记忆文件
#[tauri::command]
pub async fn list_memory_files(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<MemoryFileMeta>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    MemoryRepo::list_files_by_agent(&conn, &agent_id).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct MemoryFileEntry {
    agent_id: String,
    agent_name: String,
    file_path: String,
    file_hash: String,
    indexed_hash: Option<String>,
    chunk_count: i64,
    modified: String,
    size: u64,
    index_state: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MemoryAgentOverview {
    agent_id: String,
    agent_name: String,
    file_count: usize,
    indexed_file_count: usize,
    stale_file_count: usize,
    total_chunks: i64,
    files: Vec<MemoryFileEntry>,
}

#[derive(Debug, Serialize)]
pub struct MemoryOverview {
    agent_count: usize,
    file_count: usize,
    indexed_file_count: usize,
    stale_file_count: usize,
    total_chunks: i64,
    agents: Vec<MemoryAgentOverview>,
}

#[derive(Debug, Serialize)]
pub struct MemoryWriteResult {
    file_path: String,
    indexed_files: usize,
}

/// 读取指定 Agent 的记忆文件内容
#[tauri::command]
pub async fn get_memory_file(
    state: State<'_, AppState>,
    agent_id: String,
    file_path: String,
) -> Result<String, String> {
    let full = resolve_agent_memory_file(&state.db, &agent_id, &file_path, true)?;
    read_file_content(&full.to_string_lossy()).map_err(|e| e.to_string())
}

/// 删除记忆文件（同时清理索引）
#[tauri::command]
pub async fn delete_memory_file(
    state: State<'_, AppState>,
    agent_id: String,
    file_path: String,
) -> Result<(), String> {
    let full = resolve_agent_memory_file(&state.db, &agent_id, &file_path, true)?;
    std::fs::remove_file(&full).map_err(|e| format!("delete file: {e}"))?;
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    MemoryRepo::delete_by_file(&conn, &agent_id, &file_path).map_err(|e| e.to_string())
}

/// 返回按 Agent ID 聚合的文件与索引状态概览。
#[tauri::command]
pub async fn memory_overview(state: State<'_, AppState>) -> Result<MemoryOverview, String> {
    let agents: Vec<(String, String)> = {
        let conn = state
            .db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, name FROM agents ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let mut overview_agents = Vec::with_capacity(agents.len());
    for (agent_id, agent_name) in agents {
        let dir = ensure_agent_memory_dir(&agent_name)?;
        let indexed = {
            let conn = state
                .db
                .get_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            MemoryRepo::list_files_by_agent(&conn, &agent_id).map_err(|e| e.to_string())?
        };
        let indexed_by_path: std::collections::HashMap<_, _> = indexed
            .iter()
            .map(|meta| (meta.file_path.as_str(), meta))
            .collect();
        let mut files = Vec::new();
        for file in scan_memory_files(&dir)? {
            let file_path = rel_path(&file);
            let hash = file_hash(&file)?;
            let indexed_meta = indexed_by_path.get(file_path.as_str());
            let index_state = match indexed_meta {
                None => "missing",
                Some(meta) if meta.file_hash == hash => "current",
                Some(_) => "stale",
            };
            let metadata = file
                .metadata()
                .map_err(|e| format!("memory metadata: {e}"))?;
            let modified = metadata
                .modified()
                .ok()
                .map(|time| {
                    let dt: chrono::DateTime<chrono::Utc> = time.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();
            files.push(MemoryFileEntry {
                agent_id: agent_id.clone(),
                agent_name: agent_name.clone(),
                file_path,
                file_hash: hash,
                indexed_hash: indexed_meta.map(|meta| meta.file_hash.clone()),
                chunk_count: indexed_meta.map(|meta| meta.chunk_count).unwrap_or(0),
                modified,
                size: metadata.len(),
                index_state,
            });
        }
        let indexed_file_count = files
            .iter()
            .filter(|file| file.index_state == "current")
            .count();
        let stale_file_count = files.len().saturating_sub(indexed_file_count);
        overview_agents.push(MemoryAgentOverview {
            agent_id,
            agent_name,
            file_count: files.len(),
            indexed_file_count,
            stale_file_count,
            total_chunks: indexed.iter().map(|meta| meta.chunk_count).sum(),
            files,
        });
    }
    Ok(MemoryOverview {
        agent_count: overview_agents.len(),
        file_count: overview_agents.iter().map(|agent| agent.file_count).sum(),
        indexed_file_count: overview_agents
            .iter()
            .map(|agent| agent.indexed_file_count)
            .sum(),
        stale_file_count: overview_agents
            .iter()
            .map(|agent| agent.stale_file_count)
            .sum(),
        total_chunks: overview_agents.iter().map(|agent| agent.total_chunks).sum(),
        agents: overview_agents,
    })
}

/// 保存记忆文件内容，并在写入成功后立即同步索引。
#[tauri::command]
pub async fn save_memory_file(
    state: State<'_, AppState>,
    agent_id: String,
    file_path: String,
    content: String,
) -> Result<MemoryWriteResult, String> {
    let _guard = MemoryOperationGuard::acquire(&agent_id)?;
    let full = resolve_agent_memory_file(&state.db, &agent_id, &file_path, true)?;
    let parent = full.parent().ok_or("memory file has no parent")?;
    let temp_path = parent.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&temp_path, &content)
        .map_err(|e| format!("write temporary memory file: {e}"))?;
    if let Err(error) = std::fs::rename(&temp_path, &full) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("replace memory file: {error}"));
    }
    let indexed_files = scan_and_index_with_db(&state.db, &state.key_manager, &agent_id).await?;
    tracing::info!(agent_id = %agent_id, file = %file_path, len = content.len(), indexed_files, "memory file saved and indexed");
    Ok(MemoryWriteResult {
        file_path,
        indexed_files,
    })
}

/// 在系统文件管理器中打开指定 Agent ID 的记忆目录。
#[tauri::command]
pub async fn open_memory_dir(state: State<'_, AppState>, agent_id: String) -> Result<(), String> {
    let agent_name = agent_name_by_id(&state.db, &agent_id)?;
    let dir = ensure_agent_memory_dir(&agent_name)?;
    let path_str = dir.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(&path_str)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&path_str)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    std::process::Command::new("xdg-open")
        .arg(&path_str)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 为指定 Agent 的无标签记忆文件生成并追加标签，随后同步该 Agent 的索引。
/// LLM 失败时回退到本地关键词提取。
#[tauri::command]
pub async fn generate_memory_tags(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<usize, String> {
    let _guard = MemoryOperationGuard::acquire(&agent_id)?;
    let agent_name = agent_name_by_id(&state.db, &agent_id)?;
    let root = ensure_agent_memory_dir(&agent_name)?;
    let (api_key, api_base_url, llm_model) = {
        let api_key = crate::commands::settings::load_api_key(&state)?;
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
        (api_key, au, lm)
    };
    let mut total = 0usize;
    for path in scan_memory_files(&root)? {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (_clean, tag_opt) = parse_tag_line(&content);
        if tag_opt.is_some() {
            continue;
        }
        let tags = llm_generate_tags(&api_base_url, &api_key, &llm_model, &content)
            .await
            .unwrap_or_else(|_| {
                extract_keywords_weighted(&content)
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .collect()
            });
        if tags.is_empty() {
            continue;
        }
        use std::io::Write;
        let tag_line = format!("Tag: {}\n", tags.join(", "));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| format!("open memory file: {e}"))?;
        file.write_all(tag_line.as_bytes())
            .map_err(|e| format!("write memory tags: {e}"))?;
        total += 1;
    }
    if total > 0 {
        scan_and_index_with_db(&state.db, &state.key_manager, &agent_id).await?;
    }
    tracing::info!(%agent_id, files = total, "generate_memory_tags done");
    Ok(total)
}

/// 记忆统计
#[tauri::command]
pub async fn memory_stats(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<serde_json::Value, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let files = MemoryRepo::list_files_by_agent(&conn, &agent_id).map_err(|e| e.to_string())?;
    let total_chunks: i64 = files.iter().map(|f| f.chunk_count).sum();
    Ok(serde_json::json!({
        "file_count": files.len(),
        "total_chunks": total_chunks,
        "files": files,
    }))
}

// ---- remember 工具（AI 主动记忆）----

/// remember 工具执行器：写入时间戳命名的 .txt 文件到 dailynote/{agent}/
/// 写入后立即触发该文件的增量索引重建。
/// 读取 prompts/ 下的提示词文件，失败时返回 default_content。
fn read_prompt(filename: &str, default_content: &str) -> String {
    let path = project_root().join("prompts").join(filename);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        tracing::warn!(file = %path.display(), error = %e, "failed to read prompt file, using default");
        default_content.to_string()
    })
}

/// 用 LLM 从日记内容中提取 5-10 个关键词标签。提示词从 prompts/tag_master.txt 读取。
async fn llm_generate_tags(
    api_base_url: &str,
    api_key: &str,
    model: &str,
    text: &str,
) -> Result<Vec<String>, String> {
    let prompt = read_prompt(
        "tag_master.txt",
        "提取 5-10 个多维度关键词标签。直接输出标签，英文逗号分隔。",
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": prompt },
            { "role": "user", "content": text }
        ],
        "max_tokens": 150,
        "temperature": 0.1
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
            "memory tag provider request failed (HTTP {status})"
        ));
    }
    let resp_json: serde_json::Value = resp.json().await.map_err(|e| format!("llm parse: {e}"))?;
    let reply = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "no response".to_string())?;
    // 清理 "Tag:" 前缀（LLM 可能不听话）+ 按逗号拆分
    let cleaned = reply
        .trim()
        .trim_start_matches("Tag:")
        .trim_start_matches("tag:")
        .trim();
    let tags: Vec<String> = cleaned
        .split(['，', ',', '\n'])
        .map(|t| {
            t.trim()
                .trim_matches('"')
                .trim_matches('「')
                .trim_matches('」')
                .trim_matches('"')
                .to_lowercase()
        })
        .filter(|t| !t.is_empty() && t.len() >= 2 && t != "tag")
        .collect();
    if tags.is_empty() {
        return Err("empty tags from llm".into());
    }
    Ok(tags)
}

pub async fn exec_remember(
    db: &ripple_conversation_store::DbPool,
    key_manager: &std::sync::Arc<ripple_security::KeyManager>,
    conversation_id: &str,
    args: &serde_json::Value,
) -> Result<String, String> {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return Err("missing content to remember".into());
    }

    // 查 conversation 的 agent_id（从 metadata）
    let agent_id = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let metadata: String = conn
            .query_row(
                "SELECT metadata FROM conversations WHERE id=?1",
                [conversation_id],
                |r| r.get(0),
            )
            .map_err(|e| format!("conversation not found: {e}"))?;
        let meta: serde_json::Value = serde_json::from_str(&metadata).unwrap_or_default();
        meta.get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string()
    };

    // 查 agent name
    let agent_name = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.query_row("SELECT name FROM agents WHERE id=?1", [&agent_id], |r| {
            r.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "default".into())
    };

    // 确保目录存在 + 写入时间戳命名的 .txt 文件
    let dir = ensure_agent_memory_dir(&agent_name)?;
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let file = dir.join(format!("{ts}.txt"));
    let display_time = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let entry = format!("[{display_time}] {content}\n");
    std::fs::write(&file, &entry).map_err(|e| format!("write memory file: {e}"))?;
    tracing::info!(agent = %agent_name, file = %ts, "remember: wrote memory file");

    // 用 LLM 生成 tag（不阻塞用户：remember 是 AI 后台工具，返回确认后继续对话）
    // 失败时回退到关键词提取
    let (api_key, api_base_url, llm_model) = {
        let api_key = crate::commands::settings::load_api_key_from_pool(db, key_manager)?;
        let conn = db
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
        (api_key, au, lm)
    };
    let tags_llm = llm_generate_tags(&api_base_url, &api_key, &llm_model, content).await;
    let weighted = match &tags_llm {
        Ok(tags) => tags.iter().map(|t| (t.clone(), 1usize)).collect(),
        Err(_) => extract_keywords_weighted(content),
    };
    let tag_line = if weighted.is_empty() {
        String::new()
    } else {
        format!(
            "Tag: {}",
            weighted
                .iter()
                .map(|(t, _w)| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .map_err(|e| format!("append tags: {e}"))?;
        writeln!(f, "{}", tag_line).map_err(|e| format!("write tag line: {e}"))?;
    }

    // 索引：embedding → 加权 tags → 存 memories 表
    let rel = rel_path(&file);
    let hash = file_hash(&file)?;
    let client = EmbeddingClient::new(&api_base_url, &api_key, "Qwen/Qwen3-Embedding-8B")?;
    let full_text = format!("{}\n{}", entry.trim(), tag_line);
    let emb = tokio::time::timeout(Duration::from_secs(30), client.embed(&full_text))
        .await
        .ok()
        .and_then(|r| r.ok());
    let emb_json = emb
        .as_ref()
        .map(|e| serde_json::to_string(e).unwrap_or_default());
    let tags_json = serde_json::to_string(
        &weighted
            .iter()
            .map(|(t, w)| serde_json::json!({"t": t, "w": w}))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();
    let chunk_data = vec![(full_text.clone(), emb_json, tags_json)];
    {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        MemoryRepo::replace_file_chunks(&conn, &agent_id, &rel, &hash, &chunk_data)
            .map_err(|e| e.to_string())?;
    }
    tracing::info!(file = %rel, "remember: indexed with tags+embedding");

    Ok(format!(
        "Memory stored: {}",
        content.chars().take(80).collect::<String>()
    ))
}

// ---- 混合检索：Tag 权重 + Embedding + RRF ----

/// 混合检索：tag 权重匹配 + embedding 语义 → RRF 取 top-K。
/// 返回 (chunk, tag_weight, emb_score) 供日志/调试。
pub async fn hybrid_search_memories(
    db: &ripple_conversation_store::DbPool,
    agent_id: &str,
    query: &str,
    top_k: usize,
    api_key: &str,
    api_base_url: &str,
) -> Result<Vec<MemoryChunk>, String> {
    let kw = extract_keywords_weighted(query);
    let keywords: Vec<String> = kw.iter().map(|(t, _)| t.clone()).collect();
    if keywords.is_empty() {
        // 无关键词则回退最近 top_k 条
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        return MemoryRepo::list_recent(&conn, agent_id, top_k).map_err(|e| e.to_string());
    }

    let kw_refs: Vec<&str> = keywords.iter().map(|s| s.as_str()).collect();

    // Step 1: Tag 权重匹配
    let tag_weighted = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        MemoryRepo::search_by_tags_weighted(&conn, agent_id, &kw_refs, top_k * 3)
            .map_err(|e| e.to_string())?
    };

    // Step 2: Embedding 语义检索（为空时跳过 embedding，仅用 tag 结果）
    let all_chunks = {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        MemoryRepo::list_chunks_by_agent(&conn, agent_id).map_err(|e| e.to_string())?
    };
    let emb_results: Vec<(f64, MemoryChunk)> = if !all_chunks.is_empty() {
        let client = EmbeddingClient::new(api_base_url, api_key, "Qwen/Qwen3-Embedding-8B")?;
        let query_emb =
            match tokio::time::timeout(Duration::from_secs(15), client.embed(query)).await {
                Ok(Ok(e)) => e,
                _ => {
                    tracing::warn!("memory embed failed/timeout, falling back to tag-only");
                    return Ok(tag_weighted.into_iter().map(|(c, _w)| c).collect());
                }
            };
        let mut scored: Vec<(f64, MemoryChunk)> = Vec::new();
        for chunk in &all_chunks {
            if let Some(emb_str) = &chunk.embedding_json {
                if let Ok(emb) = serde_json::from_str::<Vec<f32>>(emb_str) {
                    let sim = ripple_rag::cosine_similarity(&query_emb, &emb);
                    scored.push((sim, chunk.clone()));
                }
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k * 3).collect()
    } else {
        vec![]
    };

    // Step 3: RRF 融合 + 共现 boost（同文件的其他 chunk 也加分）
    use std::collections::HashMap;
    let k = 60.0;
    let mut rrf: HashMap<String, (f64, MemoryChunk)> = HashMap::new();

    // 收集共现 boost 目标：tag 命中的 chunk 的同文件兄弟（取完整 chunk）
    // 用真实 chunk 数据，避免空 stub 混入最终结果
    let mut cooc_boost: HashMap<String, (usize, MemoryChunk)> = HashMap::new();
    for (chunk, _w) in &tag_weighted {
        if let Ok(sibs) = MemoryRepo::find_sibling_chunks(
            &db.get_timeout(Duration::from_secs(3))
                .map_err(|e| e.to_string())?,
            &chunk.file_path,
            &chunk.id,
        ) {
            for sib in sibs {
                cooc_boost
                    .entry(sib.id.clone())
                    .and_modify(|(c, _)| *c += 1)
                    .or_insert((1, sib));
            }
        }
    }

    for (i, (chunk, _w)) in tag_weighted.iter().enumerate() {
        rrf.entry(chunk.id.clone())
            .or_insert_with(|| (0.0, chunk.clone()))
            .0 += 1.0 / (k + i as f64 + 1.0);
    }
    for (i, (_score, chunk)) in emb_results.iter().enumerate() {
        rrf.entry(chunk.id.clone())
            .or_insert_with(|| (0.0, chunk.clone()))
            .0 += 1.0 / (k + i as f64 + 1.0);
    }
    // 共现 boost：tag 匹配的 chunk 的同文件兄弟给 RRF 加分（用真实 chunk 数据填入）
    for (sid, (cnt, chunk)) in &cooc_boost {
        let boost = (*cnt as f64) * (1.0 / (k + 5.0 + 1.0)); // 固定位置 5 的 RRF 分支
        rrf.entry(sid.clone())
            .and_modify(|(s, _)| *s += boost)
            .or_insert((boost, chunk.clone()));
    }

    let mut fused: Vec<(f64, MemoryChunk)> = rrf.into_values().collect();
    fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(fused.into_iter().take(top_k).map(|(_score, c)| c).collect())
}

/// AIMemo 模式：混合检索 top-5 + 最近 10 条 → LLM 总结 → 注入。
/// 比直接塞 chunks 更省 token、信息更精炼。
pub async fn build_memory_prompt(
    db: &ripple_conversation_store::DbPool,
    agent_id: &str,
    query: &str,
    api_key: &str,
    api_base_url: &str,
) -> Option<String> {
    let recent = {
        let conn = db.get_timeout(Duration::from_secs(5)).ok()?;
        MemoryRepo::list_recent(&conn, agent_id, 10).ok()?
    };
    if recent.is_empty() {
        return None;
    }

    let hybrid = hybrid_search_memories(db, agent_id, query, 5, api_key, api_base_url)
        .await
        .unwrap_or_default();

    // 去重合并 chunks：保留完整内容（多行记忆折叠为单行，避免截断至首行丢信息）
    let mut seen = std::collections::HashSet::new();
    let mut chunks_text = String::new();
    for chunk in hybrid.iter().chain(recent.iter()) {
        if !seen.insert(chunk.id.clone()) {
            continue;
        }
        let text = chunk.content.trim();
        if text.is_empty() {
            continue;
        }
        let flat = text.replace('\n', " ");
        chunks_text.push_str(&format!("- {}\n", flat));
    }
    if chunks_text.is_empty() {
        return None;
    }

    // LLM 总结（失败则回退到直接列 chunks）
    let llm_model = {
        let conn = db.get_timeout(Duration::from_secs(3)).ok();
        conn.and_then(|c| {
            c.query_row(
                "SELECT value FROM settings WHERE key='llm_model'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "deepseek-v4-flash".into())
    };
    let summarized = llm_summarize_memories(api_base_url, api_key, &llm_model, query, &chunks_text)
        .await
        .unwrap_or(chunks_text);
    Some(summarized)
}

/// LLM 对检索到的记忆做总结（AIMemo 模式）。提示词从 prompts/aimemo_prompt.txt 读取，
/// 支持 {MEMORY_QUERY} 和 {MEMORY_CHUNKS} 占位符。
async fn llm_summarize_memories(
    api_base_url: &str,
    api_key: &str,
    model: &str,
    query: &str,
    memories: &str,
) -> Result<String, String> {
    let prompt_template = read_prompt("aimemo_prompt.txt", "查询：{MEMORY_QUERY}\n\n记忆：\n{MEMORY_CHUNKS}\n\n提取最相关信息简要总结。无关则返回「无相关记忆」。");
    let user_content = prompt_template
        .replace("{MEMORY_QUERY}", query)
        .replace("{MEMORY_CHUNKS}", memories);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": user_content }
        ],
        "max_tokens": 500,
        "temperature": 0.2,
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
            "memory summary provider request failed (HTTP {status})"
        ));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let reply = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("no response")?;
    Ok(reply.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_memory_path;

    #[test]
    fn safe_memory_paths_reject_traversal_and_wrong_extensions() {
        let root =
            std::env::temp_dir().join(format!("ripple-memory-path-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("note.md"), "ok").unwrap();

        assert!(validate_memory_path(&root, "note.md", true).is_ok());
        assert!(validate_memory_path(&root, "../secret.md", false).is_err());
        assert!(validate_memory_path(&root, "note.json", false).is_err());
        assert!(
            validate_memory_path(&root, &root.join("note.md").to_string_lossy(), true).is_err()
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
