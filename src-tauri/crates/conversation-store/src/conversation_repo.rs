//! 对话 CRUD。

use chrono::Utc;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use ripple_core::Conversation;

use crate::error::{StoreError, StoreResult};

pub struct ConversationRepo;

impl ConversationRepo {
    /// 创建新对话
    pub fn create(
        conn: &PooledConnection<SqliteConnectionManager>,
        provider_id: &str,
        model_id: &str,
        title: Option<&str>,
        system_prompt: Option<&str>,
    ) -> StoreResult<Conversation> {
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let title = title.unwrap_or("New Conversation");

        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, model_id, provider_id, system_prompt, pinned, archived, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, '{}')",
            params![id, title, now, now, model_id, provider_id, system_prompt],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Self::get_by_id(conn, &id)
    }

    /// 按 ID 获取
    pub fn get_by_id(
        conn: &PooledConnection<SqliteConnectionManager>,
        id: &str,
    ) -> StoreResult<Conversation> {
        conn.query_row(
            "SELECT id, title, created_at, updated_at, model_id, provider_id,
                    system_prompt, pinned, archived, metadata
             FROM conversations WHERE id = ?1",
            [id],
            |r| {
                Ok(Conversation {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(2)?)
                        .map(|d| d.to_utc())
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(3)?)
                        .map(|d| d.to_utc())
                        .unwrap_or_else(|_| Utc::now()),
                    model_id: r.get(4)?,
                    provider_id: r.get(5)?,
                    system_prompt: r.get(6)?,
                    pinned: r.get::<_, i32>(7)? != 0,
                    archived: r.get::<_, i32>(8)? != 0,
                    metadata: serde_json::from_str(&r.get::<_, String>(9)?)
                        .unwrap_or_default(),
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(format!("conversation {id}")),
            _ => StoreError::Database(e.to_string()),
        })
    }

    /// 列出对话（支持模糊搜索、分页）。按 updated_at DESC 排序。
    pub fn list(
        conn: &PooledConnection<SqliteConnectionManager>,
        search: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Conversation>> {
        let (where_clause, query_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(q) = search {
                ("WHERE title LIKE ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3".into(),
                 vec![Box::new(format!("%{}%", q)), Box::new(limit as i64), Box::new(offset as i64)])
            } else {
                ("ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2".into(),
                 vec![Box::new(limit as i64), Box::new(offset as i64)])
            };

        let sql = format!("SELECT id, title, created_at, updated_at, model_id, provider_id,
                          system_prompt, pinned, archived, metadata
                   FROM conversations {where_clause}");

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = query_params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |r| {
            Ok(Conversation {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get::<_, String>(2).ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.to_utc())
                    .unwrap_or_else(|| Utc::now()),
                updated_at: r.get::<_, String>(3).ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.to_utc())
                    .unwrap_or_else(|| Utc::now()),
                model_id: r.get(4)?,
                provider_id: r.get(5)?,
                system_prompt: r.get(6)?,
                pinned: r.get::<_, i32>(7)? != 0,
                archived: r.get::<_, i32>(8)? != 0,
                metadata: r.get::<_, String>(9).ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut out = Vec::with_capacity(limit);
        for row in rows {
            out.push(row.map_err(|e| StoreError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    /// 更新对话
    pub fn update(
        conn: &PooledConnection<SqliteConnectionManager>,
        id: &str,
        title: Option<&str>,
        system_prompt: Option<&str>,
        pinned: Option<bool>,
        archived: Option<bool>,
        model_id: Option<&str>,
        provider_id: Option<&str>,
    ) -> StoreResult<Conversation> {
        let now = Utc::now().to_rfc3339();

        if let Some(v) = title {
            conn.execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![v, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }
        if let Some(v) = system_prompt {
            conn.execute(
                "UPDATE conversations SET system_prompt = ?1, updated_at = ?2 WHERE id = ?3",
                params![v, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }
        if let Some(v) = pinned {
            conn.execute(
                "UPDATE conversations SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
                params![v as i32, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }
        if let Some(v) = archived {
            conn.execute(
                "UPDATE conversations SET archived = ?1, updated_at = ?2 WHERE id = ?3",
                params![v as i32, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }
        if let Some(v) = model_id {
            conn.execute(
                "UPDATE conversations SET model_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![v, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }
        if let Some(v) = provider_id {
            conn.execute(
                "UPDATE conversations SET provider_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![v, now, id],
            ).map_err(|e| StoreError::Database(e.to_string()))?;
        }

        Self::get_by_id(conn, id)
    }

    /// 删除对话（级联删除消息）
    pub fn delete(
        conn: &PooledConnection<SqliteConnectionManager>,
        id: &str,
    ) -> StoreResult<()> {
        let affected = conn
            .execute("DELETE FROM conversations WHERE id = ?1", [id])
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if affected == 0 {
            return Err(StoreError::NotFound(format!("conversation {id}")));
        }
        Ok(())
    }

    /// 总数（可用于分页计算）
    pub fn count(
        conn: &PooledConnection<SqliteConnectionManager>,
        search: Option<&str>,
    ) -> StoreResult<usize> {
        let (sql, param): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(q) = search {
                ("SELECT COUNT(*) FROM conversations WHERE title LIKE ?1".into(),
                 vec![Box::new(format!("%{}%", q))])
            } else {
                ("SELECT COUNT(*) FROM conversations".into(), vec![])
            };

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = param.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, param_refs.as_slice(), |r| r.get(0))
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(count as usize)
    }
}
