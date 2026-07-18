//! 消息 CRUD + FTS5 全文搜索。

use chrono::Utc;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use ripple_core::{Message, MessageRole};

use crate::error::{StoreError, StoreResult};

pub struct MessageRepo;

impl MessageRepo {
    /// 插入一条消息
    pub fn insert(
        conn: &PooledConnection<SqliteConnectionManager>,
        message: &Message,
    ) -> StoreResult<()> {
        let now = message.created_at.to_rfc3339();
        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let metadata = serde_json::to_string(&message.metadata)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let role_str = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                message.id,
                message.conversation_id,
                role_str,
                content_json,
                now,
                message.token_count,
                metadata,
            ],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(())
    }

    /// 按 conversation_id 分页取消息，按创建时间正序。
    /// `before_id` 游标分页：取比指定消息更早的消息。
    pub fn list_by_conversation(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
        limit: usize,
        before_id: Option<&str>,
    ) -> StoreResult<Vec<Message>> {
        let cursor_rowid = if let Some(message_id) = before_id {
            Some(
                conn.query_row(
                    "SELECT rowid FROM messages WHERE id = ?1 AND conversation_id = ?2",
                    params![message_id, conversation_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => {
                        StoreError::NotFound(format!("message {message_id}"))
                    }
                    other => StoreError::Database(other.to_string()),
                })?,
            )
        } else {
            None
        };
        let sql = if cursor_rowid.is_some() {
            "SELECT id, conversation_id, role, content, summary, created_at, token_count, metadata FROM (
                SELECT rowid AS message_rowid, id, conversation_id, role, content, summary, created_at, token_count, metadata
                FROM messages WHERE conversation_id = ?1 AND rowid < ?2 ORDER BY rowid DESC LIMIT ?3
             ) ORDER BY message_rowid ASC"
        } else {
            "SELECT id, conversation_id, role, content, summary, created_at, token_count, metadata FROM (
                SELECT rowid AS message_rowid, id, conversation_id, role, content, summary, created_at, token_count, metadata
                FROM messages WHERE conversation_id = ?1 ORDER BY rowid DESC LIMIT ?2
             ) ORDER BY message_rowid ASC"
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut query = if let Some(rowid) = cursor_rowid {
            stmt.query(params![conversation_id, rowid, limit as i64])
        } else {
            stmt.query(params![conversation_id, limit as i64])
        }
        .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut out = Vec::with_capacity(limit);
        while let Some(row) = query
            .next()
            .map_err(|e| StoreError::Database(e.to_string()))?
        {
            let role_str: String = row
                .get(2)
                .map_err(|e| StoreError::Database(e.to_string()))?;
            let content_json: String = row
                .get(3)
                .map_err(|e| StoreError::Database(e.to_string()))?;
            out.push(Message {
                id: row
                    .get(0)
                    .map_err(|e| StoreError::Database(e.to_string()))?,
                conversation_id: row
                    .get(1)
                    .map_err(|e| StoreError::Database(e.to_string()))?,
                role: match role_str.as_str() {
                    "system" => MessageRole::System,
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    "tool" => MessageRole::Tool,
                    _ => {
                        return Err(StoreError::InvalidData(format!(
                            "unknown message role: {role_str}"
                        )))
                    }
                },
                content: serde_json::from_str(&content_json)
                    .map_err(|e| StoreError::InvalidData(e.to_string()))?,
                created_at: row
                    .get::<_, String>(5)
                    .ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.to_utc())
                    .unwrap_or_else(Utc::now),
                token_count: row.get::<_, Option<i32>>(6).ok().flatten(),
                metadata: row
                    .get::<_, String>(7)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
            });
        }
        Ok(out)
    }

    /// 全文搜索消息。返回匹配的消息内容片段。
    pub fn search(
        conn: &PooledConnection<SqliteConnectionManager>,
        query: &str,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<SearchResult>> {
        let (where_extra, extra_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(cid) = conversation_id {
                (
                    "AND fts.conversation_id = ?3".into(),
                    vec![
                        Box::new(query.to_string()),
                        Box::new(limit as i64),
                        Box::new(cid.to_string()),
                    ],
                )
            } else {
                (
                    "".into(),
                    vec![Box::new(query.to_string()), Box::new(limit as i64)],
                )
            };

        let sql = format!(
            "SELECT fts.conversation_id, m.role, snippet(messages_fts, 1, '<b>', '</b>', '…', 40), m.created_at
             FROM messages_fts fts
             JOIN messages m ON m.rowid = fts.rowid
             WHERE messages_fts MATCH ?1 {where_extra}
             ORDER BY rank
             LIMIT ?2"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            extra_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(param_refs.as_slice(), |r| {
                Ok(SearchResult {
                    conversation_id: r.get(0)?,
                    role: r.get(1)?,
                    snippet: r.get(2)?,
                    created_at: r
                        .get::<_, String>(3)
                        .ok()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|d| d.to_utc())
                        .unwrap_or_else(Utc::now),
                    match_text: String::new(),
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut out = Vec::with_capacity(limit);
        for row in rows {
            out.push(row.map_err(|e| StoreError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    /// 更新消息内容（仅允许更新 text 消息）
    pub fn update_content(
        conn: &PooledConnection<SqliteConnectionManager>,
        id: &str,
        new_content: &str,
    ) -> StoreResult<()> {
        let content_block = vec![ripple_core::ContentBlock::Text {
            text: new_content.to_string(),
        }];
        let content_json = serde_json::to_string(&content_block)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET content = ?1 WHERE id = ?2",
            params![content_json, id],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn truncate_after(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
        anchor_message_id: &str,
    ) -> StoreResult<usize> {
        Self::delete_relative(conn, conversation_id, anchor_message_id, false)
    }

    pub fn delete_from_inclusive(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
        anchor_message_id: &str,
    ) -> StoreResult<usize> {
        Self::delete_relative(conn, conversation_id, anchor_message_id, true)
    }

    fn delete_relative(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
        anchor_message_id: &str,
        inclusive: bool,
    ) -> StoreResult<usize> {
        let base_rowid = conn
            .query_row(
                "SELECT rowid FROM messages WHERE id = ?1 AND conversation_id = ?2",
                params![anchor_message_id, conversation_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    StoreError::NotFound(format!("message {anchor_message_id}"))
                }
                other => StoreError::Database(other.to_string()),
            })?;
        let operator = if inclusive { ">=" } else { ">" };
        conn.execute(
            &format!("DELETE FROM messages WHERE conversation_id = ?1 AND rowid {operator} ?2"),
            params![conversation_id, base_rowid],
        )
        .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// 获取某对话中最新一条消息，用于标题自动生成
    pub fn get_latest_message(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
    ) -> StoreResult<Option<Message>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, role, content, summary, created_at, token_count, metadata
                 FROM messages WHERE conversation_id = ?1
                 ORDER BY created_at DESC LIMIT 1",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut rows = stmt
            .query_map([conversation_id], |r| {
                let role_str: String = r.get(2)?;
                let content_json: String = r.get(3)?;
                Ok(Message {
                    id: r.get(0)?,
                    conversation_id: r.get(1)?,
                    role: match role_str.as_str() {
                        "system" => MessageRole::System,
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "tool" => MessageRole::Tool,
                        _ => MessageRole::User,
                    },
                    content: serde_json::from_str(&content_json).unwrap_or_default(),
                    created_at: r
                        .get::<_, String>(5)
                        .ok()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|d| d.to_utc())
                        .unwrap_or_else(Utc::now),
                    token_count: r.get::<_, Option<i32>>(6).ok().flatten(),
                    metadata: r
                        .get::<_, String>(7)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?;

        match rows.next() {
            Some(Ok(msg)) => Ok(Some(msg)),
            Some(Err(e)) => Err(StoreError::Database(e.to_string())),
            None => Ok(None),
        }
    }
}

/// FTS5 搜索结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub conversation_id: String,
    pub role: String,
    pub snippet: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub match_text: String,
}
