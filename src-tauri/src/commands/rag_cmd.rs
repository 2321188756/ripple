//! RAG 知识库管理命令。

use std::time::Duration;

use crate::state::AppState;
use ripple_rag::{embedding::EmbeddingClient, read_file_content, store, ChunkConfig};
use tauri::State;

#[tauri::command]
pub async fn create_kb(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<ripple_rag::KnowledgeBase, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    store::create_kb(&conn, &name, &description.unwrap_or_default())
}

#[tauri::command]
pub async fn list_kbs(state: State<'_, AppState>) -> Result<Vec<ripple_rag::KnowledgeBase>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    store::list_kbs(&conn)
}

#[tauri::command]
pub async fn delete_kb(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    store::delete_kb(&conn, &id)
}

#[tauri::command]
pub async fn list_docs(state: State<'_, AppState>, kb_id: String) -> Result<Vec<ripple_rag::Document>, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    store::list_docs(&conn, &kb_id)
}

/// 导入文件并索引（接收文件路径、API Key/URL/模型）
#[tauri::command]
pub async fn import_document(
    state: State<'_, AppState>,
    kb_id: String,
    file_path: String,
    api_key: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<ripple_rag::Document, String> {
    let base_url = api_base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    // 读取文件（PDF 用 pdf-extract 提取，其他直接 read_to_string）
    let content = read_file_content(&file_path)?;
    let file_name = std::path::Path::new(&file_path)
        .file_name().map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let file_type = std::path::Path::new(&file_path)
        .extension().map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "txt".into());

    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let doc = store::insert_document(&conn, &kb_id, &file_name, &file_type)?;
    store::update_doc_status(&conn, &doc.id, "indexing")?;

    let client = EmbeddingClient::new(&base_url, &api_key, &model)?;

    // 分块
    let chunks = ripple_rag::chunk_text(&content, &doc.id, &kb_id, &ChunkConfig::default());
    tracing::info!(count = chunks.len(), file = %file_name, "chunked");

    // 批量生成嵌入（每次最多 10 块）
    let embeddings = {
        let mut all = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(10) {
            let texts: Vec<&str> = batch.iter().map(|c| c.content.as_str()).collect();
            let embs = client.embed_batch(&texts).await?;
            all.extend(embs);
        }
        all
    };

    // 存数据库
    store::store_chunks_with_embeddings(&conn, chunks, embeddings)?;
    store::update_doc_status(&conn, &doc.id, "ready")?;

    tracing::info!(file = %file_name, "indexing complete");
    Ok(doc)
}

/// 删除文档及其分块
#[tauri::command]
pub async fn delete_document(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM chunks WHERE doc_id=?1", [&id]).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM documents WHERE id=?1", [&id]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 编辑文档内容：删除旧分块、重新分块和嵌入
#[tauri::command]
pub async fn update_document_content(
    state: State<'_, AppState>,
    id: String,
    content: String,
    api_key: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<(), String> {
    let base_url = api_base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    // 查找文档信息
    let (kb_id, file_name, file_type): (String, String, String) = conn
        .query_row(
            "SELECT kb_id, file_name, file_type FROM documents WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("doc not found: {e}"))?;

    // 删除旧分块
    conn.execute("DELETE FROM chunks WHERE doc_id = ?1", [&id])
        .map_err(|e| e.to_string())?;

    store::update_doc_status(&conn, &id, "indexing")?;

    let client = EmbeddingClient::new(&base_url, &api_key, &model)?;

    // 分块
    let chunks = ripple_rag::chunk_text(&content, &id, &kb_id, &ChunkConfig::default());
    tracing::info!(count = chunks.len(), doc = %id, "re-chunked");

    // 批量嵌入
    let embeddings = {
        let mut all = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(10) {
            let texts: Vec<&str> = batch.iter().map(|c| c.content.as_str()).collect();
            let embs = client.embed_batch(&texts).await?;
            all.extend(embs);
        }
        all
    };

    store::store_chunks_with_embeddings(&conn, chunks, embeddings)?;
    store::update_doc_status(&conn, &id, "ready")?;

    // 更新文件名（如果内容变了可能扩展名也变）
    let _ = conn.execute(
        "UPDATE documents SET file_name = ?1, file_type = ?2 WHERE id = ?3",
        rusqlite::params![file_name, file_type, id],
    );

    tracing::info!(doc = %id, "document content updated");
    Ok(())
}

/// 获取文档完整内容（按 chunk_index 拼接所有分块）
#[tauri::command]
pub async fn get_document_content(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT content FROM chunks WHERE doc_id = ?1 ORDER BY chunk_index ASC")
        .map_err(|e| e.to_string())?;
    let rows: Vec<String> = stmt
        .query_map([&id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    if rows.is_empty() {
        return Err("Document has no content (may still be indexing)".into());
    }
    Ok(rows.join(""))
}

/// 搜索知识库
#[tauri::command]
pub async fn search_kb(
    state: State<'_, AppState>,
    query: String,
    kb_id: Option<String>,
    top_k: Option<usize>,
    api_key: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<Vec<ripple_rag::SearchResult>, String> {
    let base_url = api_base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    let client = EmbeddingClient::new(&base_url, &api_key, &model)?;
    let query_emb = client.embed(&query).await?;

    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    store::hybrid_search(&conn, &query_emb, &query, kb_id.as_deref(), top_k.unwrap_or(5))
}

/// 递归导入文件夹中的所有文档
#[tauri::command]
pub async fn import_folder(
    state: State<'_, AppState>,
    kb_id: String,
    folder_path: String,
    api_key: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<Vec<ripple_rag::Document>, String> {
    let base_url = api_base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());
    let client = EmbeddingClient::new(&base_url, &api_key, &model)?;

    let mut results = Vec::new();
    let mut entries = Vec::new();

    // 递归收集文件
    collect_files(&folder_path, &mut entries).map_err(|e| format!("scan folder: {e}"))?;
    if entries.is_empty() {
        return Err("No supported files found in the folder".into());
    }

    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    for file_path in &entries {
        let content = match read_file_content(file_path) {
            Ok(c) => c,
            Err(e) => { tracing::warn!(file = %file_path, "skip read: {e}"); continue; }
        };

        let file_name = std::path::Path::new(file_path)
            .file_name().map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        let file_type = std::path::Path::new(file_path)
            .extension().map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "txt".into());

        let doc = match store::insert_document(&conn, &kb_id, &file_name, &file_type) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(file = %file_path, "insert: {e}"); continue; }
        };
        let _ = store::update_doc_status(&conn, &doc.id, "indexing");

        let chunks = ripple_rag::chunk_text(&content, &doc.id, &kb_id, &ChunkConfig::default());
        let count = chunks.len();
        let mut embedding_vec = Vec::with_capacity(count);
        let mut embed_failed = false;
        for batch in chunks.chunks(10) {
            let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
            match client.embed_batch(&texts.iter().map(|s| s.as_str()).collect::<Vec<_>>()).await {
                Ok(embs) => embedding_vec.extend(embs),
                Err(e) => {
                    // 关键：批次失败必须中止整篇文档。早期版本此处 `continue` 仅跳过该批，
                    // 后续批次仍会追加，导致 embedding_vec 长度 < chunks 长度，
                    // store_chunks_with_embeddings 的 zip 会把后续向量错配到前面的 chunk，
                    // 文档却仍标记 ready —— 静默数据损坏。
                    tracing::warn!(file = %file_path, "embed failed, aborting doc: {e}");
                    embed_failed = true;
                    break;
                }
            }
        }
        if embed_failed {
            let _ = store::update_doc_status(&conn, &doc.id, "error");
            continue;
        }

        if let Err(e) = store::store_chunks_with_embeddings(&conn, chunks, embedding_vec) {
            tracing::warn!(file = %file_path, "store chunks: {e}");
            let _ = store::update_doc_status(&conn, &doc.id, "error");
            continue;
        }
        let _ = store::update_doc_status(&conn, &doc.id, "ready");
        tracing::info!(count, file = %file_name, "imported");
        results.push(doc);
    }

    Ok(results)
}

fn collect_files(dir: &str, entries: &mut Vec<String>) -> Result<(), String> {
    let allowed = ["txt", "md", "pdf", "rs", "py", "js", "ts"];
    let dir = std::path::Path::new(dir);
    if !dir.is_dir() {
        return Err("not a directory".into());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path.to_string_lossy(), entries)?;
        } else if let Some(ext) = path.extension() {
            if allowed.contains(&ext.to_string_lossy().as_ref()) {
                entries.push(path.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}

/// 批量删除文档
#[tauri::command]
pub async fn batch_delete_documents(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() { return Ok(()); }
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
    let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    tx.execute(&format!("DELETE FROM chunks WHERE doc_id IN ({})", placeholders.join(",")), params.as_slice()).map_err(|e| e.to_string())?;

    tx.execute(&format!("DELETE FROM documents WHERE id IN ({})", placeholders.join(",")), params.as_slice()).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 重命名文档
#[tauri::command]
pub async fn rename_document(
    state: State<'_, AppState>,
    id: String,
    new_name: String,
) -> Result<ripple_rag::Document, String> {
    let conn = state.db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    let new_ext = std::path::Path::new(&new_name)
        .extension().map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "txt".into());
    conn.execute(
        "UPDATE documents SET file_name = ?1, file_type = ?2 WHERE id = ?3",
        rusqlite::params![new_name, new_ext, id],
    ).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT id, kb_id, file_name, file_type, status, created_at FROM documents WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row([&id], |r| {
        Ok(ripple_rag::Document {
            id: r.get(0)?,
            kb_id: r.get(1)?,
            file_name: r.get(2)?,
            file_type: r.get(3)?,
            status: r.get(4)?,
            created_at: r.get(5)?,
        })
    }).map_err(|e| format!("doc not found: {e}"))
}
