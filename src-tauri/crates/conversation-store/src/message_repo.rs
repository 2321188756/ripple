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
            "INSERT OR IGNORE INTO messages (id, conversation_id, role, content, created_at, token_count, metadata)
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
        let (where_extra, extra_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(bid) = before_id {
                ("AND m.created_at < COALESCE((SELECT created_at FROM messages WHERE id = ?3), '9999-12-31T23:59:59Z')".into(),
                 vec![Box::new(limit as i64), Box::new(conversation_id.to_string()), Box::new(bid.to_string())])
            } else {
                ("".into(),
                 vec![Box::new(limit as i64), Box::new(conversation_id.to_string())])
            };

        let sql = format!(
            "SELECT m.id, m.conversation_id, m.role, m.content, m.summary, m.created_at, m.token_count, m.metadata
             FROM messages m
             WHERE m.conversation_id = ?2 {where_extra}
             ORDER BY m.created_at ASC
             LIMIT ?1"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = extra_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let rows = stmt.query_map(param_refs.as_slice(), |r| {
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
                created_at: r.get::<_, String>(5).ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.to_utc())
                    .unwrap_or_else(|| Utc::now()),
                token_count: r.get::<_, Option<i32>>(6).ok().flatten(),
                metadata: r.get::<_, String>(7).ok()
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

    /// 全文搜索消息。返回匹配的消息内容片段。
    pub fn search(
        conn: &PooledConnection<SqliteConnectionManager>,
        query: &str,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<SearchResult>> {
        let (where_extra, extra_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(cid) = conversation_id {
                ("AND fts.conversation_id = ?3".into(),
                 vec![Box::new(query.to_string()), Box::new(limit as i64), Box::new(cid.to_string())])
            } else {
                ("".into(),
                 vec![Box::new(query.to_string()), Box::new(limit as i64)])
            };

        let sql = format!(
            "SELECT fts.conversation_id, m.role, snippet(messages_fts, 1, '<b>', '</b>', '…', 40), m.created_at
             FROM messages_fts fts
             JOIN messages m ON m.rowid = fts.rowid
             WHERE messages_fts MATCH ?1 {where_extra}
             ORDER BY rank
             LIMIT ?2"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = extra_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let rows = stmt.query_map(param_refs.as_slice(), |r| {
            Ok(SearchResult {
                conversation_id: r.get(0)?,
                role: r.get(1)?,
                snippet: r.get(2)?,
                created_at: r.get::<_, String>(3).ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.to_utc())
                    .unwrap_or_else(|| Utc::now()),
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
        let content_block = vec![ripple_core::ContentBlock::Text { text: new_content.to_string() }];
        let content_json = serde_json::to_string(&content_block)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET content = ?1 WHERE id = ?2",
            params![content_json, id],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    /// 删除指定对话中在某条消息之后的所有消息（含该条本身）
    ///
    /// 安全性：必须先校验 `from_message_id` 存在并取其 `created_at` 作为基准。
    /// 早期版本用 `COALESCE(..., '1970-...')` 回退，当 id 不存在时会以 1970 年为
    /// 基准命中全部消息，导致整段对话被清空。这里改为缺失即报错。
    pub fn delete_from(
        conn: &PooledConnection<SqliteConnectionManager>,
        conversation_id: &str,
        from_message_id: &str,
    ) -> StoreResult<()> {
        // 删除该消息及之后所有消息。先查 rowid，不存在则无操作。
        let rowid: Option<i64> = conn.query_row(
            "SELECT rowid FROM messages WHERE id = ?1 AND conversation_id = ?2",
            params![from_message_id, conversation_id],
            |r| r.get(0),
        ).ok();
        if let Some(base_rowid) = rowid {
            conn.execute(
                "DELETE FROM messages WHERE conversation_id = ?1 AND rowid > ?2",
                params![conversation_id, base_rowid],
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        }
        Ok(())
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
                    created_at: r.get::<_, String>(5).ok()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|d| d.to_utc())
                        .unwrap_or_else(|| Utc::now()),
                    token_count: r.get::<_, Option<i32>>(6).ok().flatten(),
                    metadata: r.get::<_, String>(7).ok()
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
