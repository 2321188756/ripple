//! 向量存储与混合检索。chunks + embeddings 存在 SQLite，检索用余弦相似度 + FTS5 融合。

use crate::embedding::{cosine_similarity, Embedding};
use crate::types::{Chunk, KnowledgeBase, Document, SearchResult};
use crate::ChunkConfig;

/// 每块最大字符数
const DEFAULT_CHUNK_SIZE: usize = 1000;
const DEFAULT_CHUNK_OVERLAP: usize = 100;

// ---- 知识库 CRUD ----

pub fn create_kb(conn: &rusqlite::Connection, name: &str, description: &str) -> Result<KnowledgeBase, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO knowledge_bases (id, name, description, chunk_size, chunk_overlap, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![id, name, description, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP, now, now],
    ).map_err(|e| e.to_string())?;
    Ok(KnowledgeBase { id, name: name.into(), description: description.into(), chunk_size: DEFAULT_CHUNK_SIZE, chunk_overlap: DEFAULT_CHUNK_OVERLAP, created_at: now })
}

pub fn list_kbs(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeBase>, String> {
    let mut stmt = conn.prepare("SELECT id, name, description, chunk_size, chunk_overlap, created_at FROM knowledge_bases ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(KnowledgeBase {
        id: r.get(0)?, name: r.get(1)?, description: r.get(2)?, chunk_size: r.get(3)?, chunk_overlap: r.get(4)?, created_at: r.get(5)?,
    })).map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for row in rows { list.push(row.map_err(|e| e.to_string())?); }
    Ok(list)
}

pub fn delete_kb(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM chunks WHERE kb_id = ?1", [id]).ok();
    conn.execute("DELETE FROM documents WHERE kb_id = ?1", [id]).ok();
    conn.execute("DELETE FROM knowledge_bases WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- 文档 CRUD ----

pub fn insert_document(conn: &rusqlite::Connection, kb_id: &str, file_name: &str, file_type: &str) -> Result<Document, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO documents (id, kb_id, file_name, file_type, status, created_at) VALUES (?1,?2,?3,?4,'pending',?5)",
        rusqlite::params![id, kb_id, file_name, file_type, now],
    ).map_err(|e| e.to_string())?;
    Ok(Document { id, kb_id: kb_id.into(), file_name: file_name.into(), file_type: file_type.into(), status: "pending".into(), created_at: now })
}

pub fn update_doc_status(conn: &rusqlite::Connection, doc_id: &str, status: &str) -> Result<(), String> {
    conn.execute("UPDATE documents SET status = ?1 WHERE id = ?2", rusqlite::params![status, doc_id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_docs(conn: &rusqlite::Connection, kb_id: &str) -> Result<Vec<Document>, String> {
    let mut stmt = conn.prepare("SELECT id, kb_id, file_name, file_type, status, created_at FROM documents WHERE kb_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([kb_id], |r| Ok(Document {
        id: r.get(0)?, kb_id: r.get(1)?, file_name: r.get(2)?, file_type: r.get(3)?, status: r.get(4)?, created_at: r.get(5)?,
    })).map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for row in rows { list.push(row.map_err(|e| e.to_string())?); }
    Ok(list)
}

// ---- 分块与嵌入 ----

/// 存储一批分块及其嵌入向量
pub fn store_chunks_with_embeddings(
    conn: &rusqlite::Connection,
    chunks: Vec<Chunk>,
    embeddings: Vec<Embedding>,
) -> Result<(), String> {
    for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
        let emb_json = serde_json::to_string(emb).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO chunks (id, doc_id, kb_id, chunk_index, content, metadata, embedding_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![chunk.id, chunk.doc_id, chunk.kb_id, chunk.chunk_index, chunk.content, chunk.metadata.to_string(), emb_json],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    if let Some(kb) = kb_id {
        sql.push_str(&format!(" AND c.kb_id = '{}'", kb.replace('\'', "''")));
    }
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| {
        let emb_str: String = r.get::<_, String>(2).ok().unwrap_or_default();
        let emb: Embedding = serde_json::from_str(&emb_str).unwrap_or_default();
        let meta_str: String = r.get::<_, String>(5).unwrap_or_default();
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, emb, r.get::<_, String>(3)?, r.get::<_, String>(4)?, meta))
    }).map_err(|e| e.to_string())?;

    let mut vec_results: Vec<(f64, String, String, String, String, serde_json::Value)> = Vec::new();
    for row in rows {
        let (id, content, emb, kb, doc_name, meta) = row.map_err(|e| e.to_string())?;
        if emb.len() != query_embedding.len() { continue; }
        let score = cosine_similarity(&emb, query_embedding);
        vec_results.push((score, id, content, kb, doc_name, meta));
    }
    // 按相似度排序，取前 top_k
    vec_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    vec_results.truncate(top_k * 2); // 多取一些供融合

    // 2. FTS5 关键词搜索
    use std::collections::HashMap;
    let mut fts_scores: HashMap<String, f64> = HashMap::new();
    let mut fts_content: HashMap<String, (String, String, serde_json::Value)> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT c.id, c.content, c.kb_id, d.file_name, c.metadata, rank FROM chunks_fts f JOIN chunks c ON c.rowid = f.rowid JOIN documents d ON c.doc_id = d.id WHERE chunks_fts MATCH ?1"
    ) {
        if let Ok(rows) = stmt.query_map([query_text], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, f64>(5)?))
        }) {
            for row in rows.flatten() {
                let (id, content, kb, doc_name, meta_str, rank) = row;
                let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
                fts_scores.insert(id.clone(), -(rank as f64)); // rank 越小越好，取反
                fts_content.insert(id, (content, doc_name, meta));
            }
        }
    }

    // 3. RRF 融合
    let k = 60.0;
    let mut rr_fused: Vec<(f64, String, String, String, String, serde_json::Value)> = Vec::new();

    // 向量结果给 RRF 分数
    for (i, (score, id, content, kb, doc_name, meta)) in vec_results.iter().enumerate() {
        let mut rrf = 1.0 / (k + i as f64 + 1.0);
        if let Some(&fts_r) = fts_scores.get(id) {
            // FTS 排名取反作为 RRF rank
            let fts_rank = (-fts_r) as usize;
            rrf += 1.0 / (k + fts_rank as f64 + 1.0);
        }
        rr_fused.push((rrf, id.clone(), content.clone(), kb.clone(), doc_name.clone(), meta.clone()));
    }

    // 只出现在 FTS 中但不在向量结果中的也要加入
    for (id, &fts_score) in &fts_scores {
        if !rr_fused.iter().any(|(_, rid, _, _, _, _)| rid == id) {
            if let Some((content, doc_name, meta)) = fts_content.get(id) {
                rr_fused.push((1.0 / (k + (-fts_score as f64) + 1.0), id.clone(), content.clone(), "".into(), doc_name.clone(), meta.clone()));
            }
        }
    }

    rr_fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    rr_fused.truncate(top_k);

    let results = rr_fused.into_iter().map(|(score, id, content, kb, doc_name, meta)| SearchResult {
        chunk_id: id, content, score, kb_id: kb, doc_name, metadata: meta,
    }).collect();

    Ok(results)
}
