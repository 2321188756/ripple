//! RAG 知识库管理命令。

use std::time::Duration;

use crate::state::AppState;
use ripple_rag::{
    embedding::{EmbeddingClient, EmbeddingProvider},
    read_file_content, store, Chunk, ChunkConfig,
};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderImportFailure {
    pub file_path: String,
    pub stage: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderImportResult {
    pub imported: Vec<ripple_rag::Document>,
    pub failures: Vec<FolderImportFailure>,
}

async fn replace_document_index<P: EmbeddingProvider + ?Sized>(
    db: &ripple_conversation_store::DbPool,
    document_id: &str,
    kb_id: &str,
    chunks: Vec<Chunk>,
    provider: &P,
) -> Result<(), String> {
    {
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        store::update_doc_status(&conn, document_id, "indexing")?;
    }

    let result = async {
        let embeddings = ripple_rag::embed_chunks(provider, &chunks).await?;
        let conn = db
            .get_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        store::replace_document_chunks(&conn, document_id, kb_id, chunks, embeddings)
    }
    .await;

    if let Err(error) = result {
        if let Ok(conn) = db.get_timeout(Duration::from_secs(5)) {
            let _ = store::update_doc_status(&conn, document_id, "error");
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub async fn create_kb(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<ripple_rag::KnowledgeBase, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    store::create_kb(&conn, &name, &description.unwrap_or_default())
}

#[tauri::command]
pub async fn list_kbs(
    state: State<'_, AppState>,
) -> Result<Vec<ripple_rag::KnowledgeBase>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    store::list_kbs(&conn)
}

#[tauri::command]
pub async fn delete_kb(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    store::delete_kb(&conn, &id)
}

#[tauri::command]
pub async fn list_docs(
    state: State<'_, AppState>,
    kb_id: String,
) -> Result<Vec<ripple_rag::Document>, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    store::list_docs(&conn, &kb_id)
}

/// 导入文件并索引（接收文件路径、API Key/URL/模型）
#[tauri::command]
pub async fn import_document(
    state: State<'_, AppState>,
    kb_id: String,
    file_path: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<ripple_rag::Document, String> {
    let api_key = crate::commands::settings::load_api_key(&state)?;
    let base_url = api_base_url
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    // 读取文件（PDF 用 pdf-extract 提取，其他直接 read_to_string）
    let content = read_file_content(&file_path)?;
    let file_name = std::path::Path::new(&file_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let file_type = std::path::Path::new(&file_path)
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "txt".into());

    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let doc = store::insert_document(&conn, &kb_id, &file_name, &file_type)?;
    drop(conn);

    let client =
        EmbeddingClient::with_client(state.http_client.clone(), &base_url, &api_key, &model);

    // 分块
    let chunks = ripple_rag::chunk_text(&content, &doc.id, &kb_id, &ChunkConfig::default());
    tracing::info!(count = chunks.len(), file = %file_name, "chunked");

    replace_document_index(&state.db, &doc.id, &kb_id, chunks, &client)
        .await
        .map_err(|error| format!("document indexing failed: {error}"))?;

    tracing::info!(file = %file_name, "indexing complete");
    Ok(doc)
}

/// 删除文档及其分块
#[tauri::command]
pub async fn delete_document(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM chunks WHERE doc_id=?1", [&id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM documents WHERE id=?1", [&id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 编辑文档内容：删除旧分块、重新分块和嵌入
#[tauri::command]
pub async fn update_document_content(
    state: State<'_, AppState>,
    id: String,
    content: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<(), String> {
    let api_key = crate::commands::settings::load_api_key(&state)?;
    let base_url = api_base_url
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;

    // 查找文档信息
    let (kb_id, file_name, file_type): (String, String, String) = conn
        .query_row(
            "SELECT kb_id, file_name, file_type FROM documents WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("doc not found: {e}"))?;
    drop(conn);

    let client =
        EmbeddingClient::with_client(state.http_client.clone(), &base_url, &api_key, &model);

    // 分块
    let chunks = ripple_rag::chunk_text(&content, &id, &kb_id, &ChunkConfig::default());
    tracing::info!(count = chunks.len(), doc = %id, "re-chunked");

    replace_document_index(&state.db, &id, &kb_id, chunks, &client)
        .await
        .map_err(|error| format!("document reindexing failed: {error}"))?;

    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE documents SET file_name = ?1, file_type = ?2 WHERE id = ?3",
        rusqlite::params![file_name, file_type, id],
    )
    .map_err(|e| e.to_string())?;

    tracing::info!(doc = %id, "document content updated");
    Ok(())
}

/// 获取文档完整内容（按 chunk_index 拼接所有分块）
#[tauri::command]
pub async fn get_document_content(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
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
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<Vec<ripple_rag::SearchResult>, String> {
    let api_key = crate::commands::settings::load_api_key(&state)?;
    let base_url = api_base_url
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());

    let client =
        EmbeddingClient::with_client(state.http_client.clone(), &base_url, &api_key, &model);
    let query_emb = tokio::time::timeout(Duration::from_secs(30), client.embed(&query))
        .await
        .map_err(|_| "embedding request timed out after 30s".to_string())??;

    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    store::hybrid_search(
        &conn,
        &query_emb,
        &query,
        kb_id.as_deref(),
        top_k.unwrap_or(5),
    )
}

/// 递归导入文件夹中的所有文档
#[tauri::command]
pub async fn import_folder(
    state: State<'_, AppState>,
    kb_id: String,
    folder_path: String,
    api_base_url: Option<String>,
    embedding_model: Option<String>,
) -> Result<FolderImportResult, String> {
    let api_key = crate::commands::settings::load_api_key(&state)?;
    let base_url = api_base_url
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let model = embedding_model.unwrap_or_else(|| "Qwen/Qwen3-Embedding-8B".into());
    let client =
        EmbeddingClient::with_client(state.http_client.clone(), &base_url, &api_key, &model);

    let mut imported = Vec::new();
    let mut failures = Vec::new();
    let mut entries = Vec::new();

    // 递归收集文件
    collect_files(&folder_path, &mut entries).map_err(|e| format!("scan folder: {e}"))?;
    if entries.is_empty() {
        return Err("No supported files found in the folder".into());
    }

    for file_path in &entries {
        let content = match read_file_content(file_path) {
            Ok(c) => c,
            Err(error) => {
                tracing::warn!(file = %file_path, error_kind = "read", "document import failed");
                failures.push(FolderImportFailure {
                    file_path: file_path.clone(),
                    stage: "read".into(),
                    error,
                });
                continue;
            }
        };

        let file_name = std::path::Path::new(file_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        let file_type = std::path::Path::new(file_path)
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "txt".into());

        let doc = {
            let conn = state
                .db
                .get_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            let doc = match store::insert_document(&conn, &kb_id, &file_name, &file_type) {
                Ok(doc) => doc,
                Err(error) => {
                    tracing::warn!(file = %file_path, error_kind = "database", "document import failed");
                    failures.push(FolderImportFailure {
                        file_path: file_path.clone(),
                        stage: "database".into(),
                        error,
                    });
                    continue;
                }
            };
            doc
        };

        let chunks = ripple_rag::chunk_text(&content, &doc.id, &kb_id, &ChunkConfig::default());
        let count = chunks.len();
        if let Err(error) =
            replace_document_index(&state.db, &doc.id, &kb_id, chunks, &client).await
        {
            tracing::warn!(file = %file_path, error_kind = "indexing", "document import failed");
            failures.push(FolderImportFailure {
                file_path: file_path.clone(),
                stage: "indexing".into(),
                error,
            });
            continue;
        }
        tracing::info!(count, file = %file_name, "imported");
        imported.push(doc);
    }

    Ok(FolderImportResult { imported, failures })
}

fn collect_files(dir: &str, entries: &mut Vec<String>) -> Result<(), String> {
    collect_files_inner(std::path::Path::new(dir), entries, 0)
}

fn collect_files_inner(
    dir: &std::path::Path,
    entries: &mut Vec<String>,
    depth: usize,
) -> Result<(), String> {
    const MAX_DEPTH: usize = 16;
    const MAX_FILES: usize = 5_000;
    if depth > MAX_DEPTH {
        return Err(format!("folder nesting exceeds {MAX_DEPTH}"));
    }
    let allowed = ["txt", "md", "pdf", "rs", "py", "js", "ts"];
    if !dir.is_dir() {
        return Err("not a directory".into());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if path.is_dir() {
            collect_files_inner(&path, entries, depth + 1)?;
        } else if let Some(ext) = path.extension() {
            if allowed.contains(&ext.to_string_lossy().to_ascii_lowercase().as_str()) {
                if entries.len() >= MAX_FILES {
                    return Err(format!(
                        "folder contains more than {MAX_FILES} supported files"
                    ));
                }
                entries.push(path.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}

/// 批量删除文档
#[tauri::command]
pub async fn batch_delete_documents(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
    let params: Vec<&dyn rusqlite::types::ToSql> = ids
        .iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();
    tx.execute(
        &format!(
            "DELETE FROM chunks WHERE doc_id IN ({})",
            placeholders.join(",")
        ),
        params.as_slice(),
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        &format!(
            "DELETE FROM documents WHERE id IN ({})",
            placeholders.join(",")
        ),
        params.as_slice(),
    )
    .map_err(|e| e.to_string())?;

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
    let conn = state
        .db
        .get_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;

    let new_ext = std::path::Path::new(&new_name)
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "txt".into());
    conn.execute(
        "UPDATE documents SET file_name = ?1, file_type = ?2 WHERE id = ?3",
        rusqlite::params![new_name, new_ext, id],
    )
    .map_err(|e| e.to_string())?;

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
    })
    .map_err(|e| format!("doc not found: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDb {
        pool: ripple_conversation_store::DbPool,
        path: std::path::PathBuf,
    }

    impl TestDb {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("ripple-rag-command-{}.db", uuid::Uuid::new_v4()));
            let pool = ripple_conversation_store::init_db(&path).unwrap();
            Self { pool, path }
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    struct FailSecondBatch {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl EmbeddingProvider for FailSecondBatch {
        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> Result<Vec<ripple_rag::embedding::Embedding>, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
                Err("batch failure".into())
            } else {
                Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
            }
        }
    }

    #[tokio::test]
    async fn failed_reindex_sets_error_without_partial_chunks() {
        let db = TestDb::new();
        let conn = db.pool.get().unwrap();
        let kb = store::create_kb(&conn, "test", "").unwrap();
        let doc = store::insert_document(&conn, &kb.id, "test.txt", "txt").unwrap();
        drop(conn);

        let chunks: Vec<Chunk> = (0..11)
            .map(|index| Chunk {
                id: format!("chunk-{index}"),
                doc_id: doc.id.clone(),
                kb_id: kb.id.clone(),
                chunk_index: index,
                content: format!("content-{index}"),
                metadata: serde_json::json!({}),
            })
            .collect();
        let provider = FailSecondBatch {
            calls: AtomicUsize::new(0),
        };

        assert!(
            replace_document_index(&db.pool, &doc.id, &kb.id, chunks, &provider)
                .await
                .is_err()
        );
        let conn = db.pool.get().unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM documents WHERE id = ?1",
                [&doc.id],
                |row| row.get(0),
            )
            .unwrap();
        let chunk_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM chunks WHERE doc_id = ?1",
                [&doc.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "error");
        assert_eq!(chunk_count, 0);
    }
}
