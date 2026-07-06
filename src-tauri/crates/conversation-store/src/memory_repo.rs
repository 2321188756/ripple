//! Agent 记忆存储 CRUD。每条记忆是 dailynote/{agent}/ 下文件的一个 chunk，
//! 带 embedding_json 向量 + memories_fts 关键词索引。

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{StoreError, StoreResult};

/// 一条记忆 chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryChunk {
    pub id: String,
    pub agent_id: String,
    pub file_path: String,
    pub file_hash: String,
    pub chunk_index: i64,
    pub content: String,
    pub embedding_json: Option<String>,
    pub tags: String,          // JSON 数组：[{"t":"tag","w":1},...]（t=标签名，w=权重）
    pub created_at: String,
    pub updated_at: String,
}

/// 文件级元信息（用于增量重建 + 前端列表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFileMeta {
    pub file_path: String,
    pub file_hash: String,
    pub chunk_count: i64,
    pub updated_at: String,
}

pub struct MemoryRepo;

impl MemoryRepo {
    /// 批量插入一个文件的所有 chunk（先删旧的）。
    /// `chunks` 每项为 (content, embedding_json, tags_json)，tags 如 '["tag1","tag2"]'。
    pub fn replace_file_chunks(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
        file_path: &str,
        file_hash: &str,
        chunks: &[(String, Option<String>, String)],
    ) -> StoreResult<()> {
        // 在 DELETE 之前读取文件首次索引的 created_at，重索引（内容变更）时只更新 updated_at，
        // 避免 list_recent 把重索引过的旧记忆排在最前。
        let created_at: String = conn
            .query_row(
                "SELECT created_at FROM memories WHERE agent_id = ?1 AND file_path = ?2 LIMIT 1",
                params![agent_id, file_path],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
        let now = chrono::Utc::now().to_rfc3339();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        tx.execute(
            "DELETE FROM memories WHERE agent_id = ?1 AND file_path = ?2",
            params![agent_id, file_path],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        for (idx, (content, emb, tags)) in chunks.iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO memories (id, agent_id, file_path, file_hash, chunk_index, content, embedding_json, tags, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, agent_id, file_path, file_hash, idx as i64, content, emb, tags, created_at, now],
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    /// 删除一个文件的所有 chunk（文件被删时清理）
    pub fn delete_by_file(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
        file_path: &str,
    ) -> StoreResult<()> {
        conn.execute(
            "DELETE FROM memories WHERE agent_id = ?1 AND file_path = ?2",
            params![agent_id, file_path],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    /// 删除某 Agent 的所有记忆（Agent 删除时清理）
    pub fn delete_by_agent(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
    ) -> StoreResult<()> {
        conn.execute("DELETE FROM memories WHERE agent_id = ?1", params![agent_id])
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    /// 获取某文件当前存储的 hash（用于增量重建检测）
    pub fn get_file_hash(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
        file_path: &str,
    ) -> StoreResult<Option<String>> {
        let result: rusqlite::Result<String> = conn.query_row(
            "SELECT file_hash FROM memories WHERE agent_id = ?1 AND file_path = ?2 LIMIT 1",
            params![agent_id, file_path],
            |r| r.get(0),
        );
        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::Database(e.to_string())),
        }
    }

    /// 列出某 Agent 的所有记忆文件元信息（含 chunk 数）
    pub fn list_files_by_agent(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
    ) -> StoreResult<Vec<MemoryFileMeta>> {
        let mut stmt = conn
            .prepare(
                "SELECT file_path, file_hash, COUNT(*) as cnt, MAX(updated_at) as latest
                 FROM memories WHERE agent_id = ?1
                 GROUP BY file_path, file_hash
                 ORDER BY file_path",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![agent_id], |r| {
                Ok(MemoryFileMeta {
                    file_path: r.get(0)?,
                    file_hash: r.get(1)?,
                    chunk_count: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            out.push(row);
        }
        Ok(out)
    }

    /// 列出某 Agent 的所有 chunk
    pub fn list_chunks_by_agent(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
    ) -> StoreResult<Vec<MemoryChunk>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, file_path, file_hash, chunk_index, content, embedding_json, tags, created_at, updated_at
                 FROM memories WHERE agent_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let row_fn = |r: &rusqlite::Row| {
            Ok(MemoryChunk {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                file_path: r.get(2)?,
                file_hash: r.get(3)?,
                chunk_index: r.get(4)?,
                content: r.get(5)?,
                embedding_json: r.get(6)?,
                tags: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        };
        let rows = stmt.query_map(params![agent_id], row_fn)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut out: Vec<MemoryChunk> = Vec::new();
        for row in rows.flatten() { out.push(row); }
        Ok(out)
    }

    /// 查询同文件的其他 chunk（完整数据，用于共现 boost）。
    /// 返回完整 MemoryChunk 而非仅 id，避免调用方用空字段 stub 占位导致空内容混入结果。
    pub fn find_sibling_chunks(
        conn: &PooledConnection<SqliteConnectionManager>,
        file_path: &str,
        exclude_id: &str,
    ) -> StoreResult<Vec<MemoryChunk>> {
        let mut stmt = conn
            .prepare("SELECT id, agent_id, file_path, file_hash, chunk_index, content, embedding_json, tags, created_at, updated_at
                      FROM memories WHERE file_path = ?1 AND id != ?2 ORDER BY chunk_index")
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let row_fn = |r: &rusqlite::Row| {
            Ok(MemoryChunk {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                file_path: r.get(2)?,
                file_hash: r.get(3)?,
                chunk_index: r.get(4)?,
                content: r.get(5)?,
                embedding_json: r.get(6)?,
                tags: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        };
        let rows = stmt.query_map(params![file_path, exclude_id], row_fn)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            out.push(row);
        }
        Ok(out)
    }

    /// 按 tag 关键词检索（含权重）：加载匹配的 chunks，Rust 端解析 tags JSON → 计算加权得分 → 排序。
    /// tags 格式：`[{"t":"tag","w":1},...]`，每个 keyword 匹配 `tags` 中的 `t` 字段并累加 `w` 值。
    pub fn search_by_tags_weighted(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
        keywords: &[&str],
        limit: usize,
    ) -> StoreResult<Vec<(MemoryChunk, usize)>> {
        if keywords.is_empty() {
            return Ok(vec![]);
        }
        // LIKE 过滤：匹配任一词即可
        let conditions: Vec<String> = (0..keywords.len())
            .map(|i| format!("tags LIKE ?{}", i + 2))
            .collect();
        let sql = format!(
            "SELECT id, agent_id, file_path, file_hash, chunk_index, content, embedding_json, tags, created_at, updated_at
             FROM memories WHERE agent_id = ?1 AND ({})",
            conditions.join(" OR "),
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| StoreError::Database(e.to_string()))?;
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(agent_id.to_string())];
        for kw in keywords {
            params.push(Box::new(format!("%{}%", kw)));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let row_fn = |r: &rusqlite::Row| {
            Ok(MemoryChunk {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                file_path: r.get(2)?,
                file_hash: r.get(3)?,
                chunk_index: r.get(4)?,
                content: r.get(5)?,
                embedding_json: r.get(6)?,
                tags: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        };
        let mut results: Vec<(MemoryChunk, usize)> = Vec::new();
        for row in stmt.query_map(param_refs.as_slice(), row_fn)
            .map_err(|e| StoreError::Database(e.to_string()))?
            .flatten()
        {
            // 解析 tags JSON → [{"t":"","w":N}]
            let weight: usize = if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&row.tags) {
                parsed.iter().filter_map(|v| {
                    let tag = v.get("t").and_then(|t| t.as_str())?;
                    let w = v.get("w").and_then(|w| w.as_u64()).unwrap_or(1) as usize;
                    // 匹配任一词则累加权重
                    keywords.iter().any(|kw| tag.contains(*kw)).then_some(w)
                }).sum()
            } else {
                0
            };
            if weight > 0 {
                results.push((row, weight));
            }
        }
        // 按权重降序
        results.sort_by(|a, b| b.1.cmp(&a.1));
        if results.len() > limit { results.truncate(limit); }
        Ok(results)
    }

    /// 最近 N 条记忆（按 created_at DESC，用于 {MEMORIES} 全量注入）
    pub fn list_recent(
        conn: &PooledConnection<SqliteConnectionManager>,
        agent_id: &str,
        limit: usize,
    ) -> StoreResult<Vec<MemoryChunk>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, file_path, file_hash, chunk_index, content, embedding_json, tags, created_at, updated_at
                 FROM memories WHERE agent_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let row_fn = |r: &rusqlite::Row| {
            Ok(MemoryChunk {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                file_path: r.get(2)?,
                file_hash: r.get(3)?,
                chunk_index: r.get(4)?,
                content: r.get(5)?,
                embedding_json: r.get(6)?,
                tags: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        };
        let rows = stmt.query_map(params![agent_id, limit as i64], row_fn)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut out: Vec<MemoryChunk> = Vec::new();
        for row in rows.flatten() { out.push(row); }
        Ok(out)
    }
}
