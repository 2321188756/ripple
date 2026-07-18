//! 向量存储与混合检索。chunks + embeddings 存在 SQLite，检索用余弦相似度 + FTS5 融合。

use crate::embedding::{cosine_similarity, Embedding};
use crate::types::{Chunk, Document, KnowledgeBase, SearchResult};

/// 每块最大字符数
const DEFAULT_CHUNK_SIZE: usize = 1000;
const DEFAULT_CHUNK_OVERLAP: usize = 100;

// ---- 知识库 CRUD ----

pub fn create_kb(
    conn: &rusqlite::Connection,
    name: &str,
    description: &str,
) -> Result<KnowledgeBase, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO knowledge_bases (id, name, description, chunk_size, chunk_overlap, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![id, name, description, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP, now, now],
    ).map_err(|e| e.to_string())?;
    Ok(KnowledgeBase {
        id,
        name: name.into(),
        description: description.into(),
        chunk_size: DEFAULT_CHUNK_SIZE,
        chunk_overlap: DEFAULT_CHUNK_OVERLAP,
        created_at: now,
    })
}

pub fn list_kbs(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeBase>, String> {
    let mut stmt = conn.prepare("SELECT id, name, description, chunk_size, chunk_overlap, created_at FROM knowledge_bases ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(KnowledgeBase {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                chunk_size: r.get(3)?,
                chunk_overlap: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

pub fn delete_kb(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
    // 事务保证一致性；早期版本用 .ok() 吞错，可能出现 KB 行已删但 chunks/documents 残留。
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM chunks WHERE kb_id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM documents WHERE kb_id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM knowledge_bases WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ---- 文档 CRUD ----

pub fn insert_document(
    conn: &rusqlite::Connection,
    kb_id: &str,
    file_name: &str,
    file_type: &str,
) -> Result<Document, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO documents (id, kb_id, file_name, file_type, status, created_at) VALUES (?1,?2,?3,?4,'pending',?5)",
        rusqlite::params![id, kb_id, file_name, file_type, now],
    ).map_err(|e| e.to_string())?;
    Ok(Document {
        id,
        kb_id: kb_id.into(),
        file_name: file_name.into(),
        file_type: file_type.into(),
        status: "pending".into(),
        created_at: now,
    })
}

pub fn update_doc_status(
    conn: &rusqlite::Connection,
    doc_id: &str,
    status: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE documents SET status = ?1 WHERE id = ?2",
        rusqlite::params![status, doc_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_docs(conn: &rusqlite::Connection, kb_id: &str) -> Result<Vec<Document>, String> {
    let mut stmt = conn.prepare("SELECT id, kb_id, file_name, file_type, status, created_at FROM documents WHERE kb_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([kb_id], |r| {
            Ok(Document {
                id: r.get(0)?,
                kb_id: r.get(1)?,
                file_name: r.get(2)?,
                file_type: r.get(3)?,
                status: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

// ---- 分块与嵌入 ----

/// 存储一批分块及其嵌入向量
pub fn store_chunks_with_embeddings(
    conn: &rusqlite::Connection,
    chunks: Vec<Chunk>,
    embeddings: Vec<Embedding>,
) -> Result<(), String> {
    if chunks.len() != embeddings.len() {
        return Err(format!(
            "chunk/embedding count mismatch: {} chunks, {} embeddings",
            chunks.len(),
            embeddings.len(),
        ));
    }
    if let Some(expected_dim) = embeddings.first().map(Vec::len) {
        if expected_dim == 0
            || embeddings
                .iter()
                .any(|embedding| embedding.len() != expected_dim)
        {
            return Err("embedding dimensions must be non-zero and consistent".into());
        }
    }
    let serialized = chunks
        .iter()
        .zip(&embeddings)
        .map(|(chunk, embedding)| {
            serde_json::to_string(embedding)
                .map(|json| (chunk, json))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for (chunk, emb_json) in serialized {
        tx.execute(
            "INSERT INTO chunks (id, doc_id, kb_id, chunk_index, content, metadata, embedding_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![chunk.id, chunk.doc_id, chunk.kb_id, chunk.chunk_index, chunk.content, chunk.metadata.to_string(), emb_json],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn replace_document_chunks(
    conn: &rusqlite::Connection,
    document_id: &str,
    expected_kb_id: &str,
    chunks: Vec<Chunk>,
    embeddings: Vec<Embedding>,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Err("document must contain at least one chunk".into());
    }
    if chunks
        .iter()
        .any(|chunk| chunk.doc_id != document_id || chunk.kb_id != expected_kb_id)
    {
        return Err("chunk document or knowledge-base mismatch".into());
    }
    if chunks
        .iter()
        .enumerate()
        .any(|(index, chunk)| chunk.chunk_index != index)
    {
        return Err("chunk indexes must be contiguous".into());
    }
    if chunks.len() != embeddings.len() {
        return Err(format!(
            "chunk/embedding count mismatch: {} chunks, {} embeddings",
            chunks.len(),
            embeddings.len()
        ));
    }
    let dimension = embeddings.first().map(Vec::len).unwrap_or_default();
    if dimension == 0
        || embeddings
            .iter()
            .any(|embedding| embedding.len() != dimension)
    {
        return Err("embedding dimensions must be non-zero and consistent".into());
    }
    let serialized = chunks
        .into_iter()
        .zip(embeddings)
        .map(|(chunk, embedding)| {
            serde_json::to_string(&embedding)
                .map(|json| (chunk, json))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let actual_kb: String = tx
        .query_row(
            "SELECT kb_id FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("document not found: {e}"))?;
    if actual_kb != expected_kb_id {
        return Err("document knowledge-base mismatch".into());
    }
    tx.execute("DELETE FROM chunks WHERE doc_id = ?1", [document_id])
        .map_err(|e| e.to_string())?;
    for (chunk, embedding_json) in serialized {
        tx.execute(
            "INSERT INTO chunks (id, doc_id, kb_id, chunk_index, content, metadata, embedding_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![chunk.id, chunk.doc_id, chunk.kb_id, chunk.chunk_index, chunk.content, chunk.metadata.to_string(), embedding_json],
        ).map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE documents SET status = 'ready' WHERE id = ?1",
        [document_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

// ---- 检索 ----

/// 混合检索：向量余弦相似度 + FTS5 关键词
pub fn hybrid_search(
    conn: &rusqlite::Connection,
    query_embedding: &Embedding,
    query_text: &str,
    kb_id: Option<&str>,
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    // 1. 加载所有 chunks + embeddings
    let mut sql = "SELECT c.id, c.content, c.embedding_json, c.kb_id, d.file_name, c.metadata FROM chunks c JOIN documents d ON c.doc_id = d.id WHERE c.embedding_json IS NOT NULL".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(kb) = kb_id {
        sql.push_str(" AND c.kb_id = ?1");
        params.push(Box::new(kb.to_string()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(param_refs.as_slice(), |r| {
            let emb_str: String = r.get::<_, String>(2).ok().unwrap_or_default();
            let emb: Embedding = serde_json::from_str(&emb_str).unwrap_or_default();
            let meta_str: String = r.get::<_, String>(5).unwrap_or_default();
            let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                emb,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                meta,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut vec_results: Vec<(f64, String, String, String, String, serde_json::Value)> = Vec::new();
    for row in rows {
        let (id, content, emb, kb, doc_name, meta) = row.map_err(|e| e.to_string())?;
        if emb.len() != query_embedding.len() {
            continue;
        }
        let score = cosine_similarity(&emb, query_embedding);
        vec_results.push((score, id, content, kb, doc_name, meta));
    }
    // 按相似度排序，取前 top_k
    vec_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    vec_results.truncate(top_k * 2); // 多取一些供融合

    // 2. FTS5 关键词搜索（ORDER BY rank = BM25 最佳匹配在前；用结果位置作为 RRF 排名）
    use std::collections::HashMap;
    let mut fts_rank_pos: HashMap<String, usize> = HashMap::new();
    let mut fts_content: HashMap<String, (String, String, String, serde_json::Value)> =
        HashMap::new();
    let fts_sql = if kb_id.is_some() {
        "SELECT c.id, c.content, c.kb_id, d.file_name, c.metadata FROM chunks_fts f JOIN chunks c ON c.rowid = f.rowid JOIN documents d ON c.doc_id = d.id WHERE chunks_fts MATCH ?1 AND c.kb_id = ?2 ORDER BY rank LIMIT ?3"
    } else {
        "SELECT c.id, c.content, c.kb_id, d.file_name, c.metadata FROM chunks_fts f JOIN chunks c ON c.rowid = f.rowid JOIN documents d ON c.doc_id = d.id WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2"
    };
    if let Ok(mut stmt) = conn.prepare(fts_sql) {
        let mut query_and_collect =
            |params: &[&dyn rusqlite::types::ToSql]| -> Result<(), rusqlite::Error> {
                let rows = stmt.query_map(params, |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })?;
                for (pos, row) in rows.flatten().enumerate() {
                    let (id, content, kb, doc_name, meta_str) = row;
                    let meta: serde_json::Value =
                        serde_json::from_str(&meta_str).unwrap_or_default();
                    fts_rank_pos.insert(id.clone(), pos);
                    fts_content.insert(id, (content, kb, doc_name, meta));
                }
                Ok(())
            };
        let limit = (top_k * 2) as i64;
        if let Some(kb) = kb_id {
            let params: [&dyn rusqlite::types::ToSql; 3] = [&query_text, &kb, &limit];
            let _ = query_and_collect(&params);
        } else {
            let params: [&dyn rusqlite::types::ToSql; 2] = [&query_text, &limit];
            let _ = query_and_collect(&params);
        }
    }

    // 3. RRF 融合
    // 注意：必须用 FTS 结果的「排名位置」参与 RRF。早期版本存的是 -(rank as f64)，
    // 而 FTS5 BM25 的 rank 是负数（越负越好），再 `(-fts_r) as usize` 会把负浮点饱和为 0，
    // 导致所有 FTS 命中拿到相同 RRF 贡献，关键词排序信号完全丢失。
    let k = 60.0;
    let mut rr_fused: Vec<(f64, String, String, String, String, serde_json::Value)> = Vec::new();

    // 向量结果给 RRF 分数
    for (i, (_score, id, content, kb, doc_name, meta)) in vec_results.iter().enumerate() {
        let mut rrf = 1.0 / (k + i as f64 + 1.0);
        if let Some(&pos) = fts_rank_pos.get(id) {
            rrf += 1.0 / (k + pos as f64 + 1.0);
        }
        rr_fused.push((
            rrf,
            id.clone(),
            content.clone(),
            kb.clone(),
            doc_name.clone(),
            meta.clone(),
        ));
    }

    // 只出现在 FTS 中但不在向量结果中的也要加入
    for (id, &pos) in &fts_rank_pos {
        if !rr_fused.iter().any(|(_, rid, _, _, _, _)| rid == id) {
            if let Some((content, kb, doc_name, meta)) = fts_content.get(id) {
                rr_fused.push((
                    1.0 / (k + pos as f64 + 1.0),
                    id.clone(),
                    content.clone(),
                    kb.clone(),
                    doc_name.clone(),
                    meta.clone(),
                ));
            }
        }
    }

    rr_fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    rr_fused.truncate(top_k);

    let results = rr_fused
        .into_iter()
        .map(|(score, id, content, kb, doc_name, meta)| SearchResult {
            chunk_id: id,
            content,
            score,
            kb_id: kb,
            doc_name,
            metadata: meta,
        })
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDb {
        pool: ripple_conversation_store::DbPool,
        path: std::path::PathBuf,
    }

    impl TestDb {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("ripple-rag-store-{}.db", uuid::Uuid::new_v4()));
            let pool = ripple_conversation_store::init_db(&path).unwrap();
            Self { pool, path }
        }

        fn conn(&self) -> r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager> {
            self.pool.get().unwrap()
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    fn one_chunk(document_id: &str, kb_id: &str, content: &str) -> Vec<Chunk> {
        vec![Chunk {
            id: uuid::Uuid::new_v4().to_string(),
            doc_id: document_id.to_string(),
            kb_id: kb_id.to_string(),
            chunk_index: 0,
            content: content.to_string(),
            metadata: serde_json::json!({}),
        }]
    }

    #[test]
    fn scoped_hybrid_search_never_returns_other_knowledge_base() {
        let db = TestDb::new();
        let conn = db.conn();
        let kb_a = create_kb(&conn, "A", "").unwrap();
        let kb_b = create_kb(&conn, "B", "").unwrap();
        let doc_a = insert_document(&conn, &kb_a.id, "a.txt", "txt").unwrap();
        let doc_b = insert_document(&conn, &kb_b.id, "b.txt", "txt").unwrap();
        replace_document_chunks(
            &conn,
            &doc_a.id,
            &kb_a.id,
            one_chunk(&doc_a.id, &kb_a.id, "shared needle alpha"),
            vec![vec![1.0, 0.0]],
        )
        .unwrap();
        replace_document_chunks(
            &conn,
            &doc_b.id,
            &kb_b.id,
            one_chunk(&doc_b.id, &kb_b.id, "shared needle beta"),
            vec![vec![1.0, 0.0]],
        )
        .unwrap();

        let query_embedding = vec![1.0, 0.0];
        let scoped =
            hybrid_search(&conn, &query_embedding, "shared needle", Some(&kb_a.id), 5).unwrap();
        assert!(!scoped.is_empty());
        assert!(scoped.iter().all(|result| result.kb_id == kb_a.id));

        let unscoped = hybrid_search(&conn, &query_embedding, "shared needle", None, 5).unwrap();
        assert!(unscoped.iter().any(|result| result.kb_id == kb_a.id));
        assert!(unscoped.iter().any(|result| result.kb_id == kb_b.id));
    }

    #[test]
    fn invalid_replacement_preserves_existing_ready_chunks() {
        let db = TestDb::new();
        let conn = db.conn();
        let kb = create_kb(&conn, "A", "").unwrap();
        let doc = insert_document(&conn, &kb.id, "a.txt", "txt").unwrap();
        replace_document_chunks(
            &conn,
            &doc.id,
            &kb.id,
            one_chunk(&doc.id, &kb.id, "old stable content"),
            vec![vec![1.0, 0.0]],
        )
        .unwrap();

        let error = replace_document_chunks(
            &conn,
            &doc.id,
            &kb.id,
            one_chunk(&doc.id, &kb.id, "new invalid content"),
            vec![vec![]],
        )
        .unwrap_err();
        assert!(error.contains("dimensions"));
        let content: String = conn
            .query_row(
                "SELECT content FROM chunks WHERE doc_id = ?1",
                [&doc.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "old stable content");
        let status: String = conn
            .query_row(
                "SELECT status FROM documents WHERE id = ?1",
                [&doc.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "ready");
    }

    #[test]
    fn replacement_and_delete_keep_real_fts_index_consistent() {
        let db = TestDb::new();
        let conn = db.conn();
        let kb = create_kb(&conn, "A", "").unwrap();
        let doc = insert_document(&conn, &kb.id, "a.txt", "txt").unwrap();
        replace_document_chunks(
            &conn,
            &doc.id,
            &kb.id,
            one_chunk(&doc.id, &kb.id, "olduniqueterm"),
            vec![vec![1.0, 0.0]],
        )
        .unwrap();
        assert_eq!(
            hybrid_search(&conn, &vec![1.0, 0.0], "olduniqueterm", Some(&kb.id), 5)
                .unwrap()
                .len(),
            1
        );

        replace_document_chunks(
            &conn,
            &doc.id,
            &kb.id,
            one_chunk(&doc.id, &kb.id, "newuniqueterm"),
            vec![vec![1.0, 0.0]],
        )
        .unwrap();
        let old_fts_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'olduniqueterm'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let new_fts_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'newuniqueterm'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_fts_count, 0);
        assert_eq!(new_fts_count, 1);

        delete_kb(&conn, &kb.id).unwrap();
        let remaining_fts_count: i64 = conn
            .query_row("SELECT count(*) FROM chunks_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining_fts_count, 0);
    }
}
