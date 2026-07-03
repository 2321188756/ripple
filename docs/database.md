# 数据库设计

SQLite 单文件存储所有数据。WAL 模式 + FTS5 全文搜索 + 连接池（r2d2，max 8）。**不使用 sqlite-vec**——嵌入向量以 JSON 存 `chunks.embedding_json` 列，检索时 Rust 端 `cosine_similarity` 计算。

## 初始化

```rust
// crates/conversation-store/src/db.rs
pub fn init_db(db_path: &Path) -> StoreResult<DbPool> {
    // PRAGMA 是 per-connection 的，必须用 with_init 对池中每条连接都设置，
    // 否则只有首个连接 foreign_keys=ON，其余连接的 ON DELETE CASCADE 静默失效。
    let manager = SqliteConnectionManager::file(db_path).with_init(|c| {
        c.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;"
        )
    });
    let pool = Pool::builder().max_size(8).build(manager)?;
    // 迁移只需在任一连接执行一次
    { let conn = pool.get()?; migration::run_migrations(&conn)?; }
    Ok(pool)
}
```

## Schema

### 对话

```sql
CREATE TABLE conversations (
    id            TEXT PRIMARY KEY NOT NULL,
    title         TEXT NOT NULL DEFAULT 'New Conversation',
    created_at    TEXT NOT NULL,           -- ISO 8601
    updated_at    TEXT NOT NULL,
    model_id      TEXT NOT NULL,
    provider_id   TEXT NOT NULL,
    system_prompt TEXT,
    pinned        INTEGER NOT NULL DEFAULT 0,
    archived      INTEGER NOT NULL DEFAULT 0,
    metadata      TEXT NOT NULL DEFAULT '{}'  -- JSON；Agent 会话带 {"agent_id":"..."}
);

CREATE INDEX idx_conv_updated ON conversations(updated_at DESC);
CREATE INDEX idx_conv_pinned  ON conversations(pinned) WHERE pinned = 1;
```

> **Agent 会话过滤**：`list_conversations(agent_id)` 用 `metadata LIKE '%"agent_id":"<id>"%'` 过滤。

### 消息

```sql
CREATE TABLE messages (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    role            TEXT NOT NULL CHECK(role IN ('system','user','assistant','tool')),
    content         TEXT NOT NULL,         -- JSON: ContentBlock 数组
    summary         TEXT,                  -- 摘要缓存（上下文裁剪用）
    created_at      TEXT NOT NULL,
    token_count     INTEGER,
    metadata        TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX idx_msg_conv ON messages(conversation_id, created_at);

-- 全文搜索（standalone fts5，非 external-content）
CREATE VIRTUAL TABLE messages_fts USING fts5(
    conversation_id UNINDEXED,
    content_text
);

-- INSERT 触发器（MIGRATION_001）：只索引首块 text
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, conversation_id, content_text)
    VALUES (new.rowid, new.conversation_id, json_extract(new.content, '$[0].text'));
END;
-- DELETE/UPDATE 触发器由 MIGRATION_005 补齐（见下）
```

### delete_from（安全删除）

```rust
// crates/conversation-store/src/message_repo.rs
// 删除某消息及其后所有消息（含本身）。必须先校验 from_message_id 存在：
pub fn delete_from(conn, conversation_id, from_message_id) -> StoreResult<()> {
    let base_created: String = conn.query_row(
        "SELECT created_at FROM messages WHERE id = ?1 AND conversation_id = ?2",
        params![from_message_id, conversation_id], |r| r.get(0),
    ).map_err(|e| match e {
        QueryReturnedNoRows => NotFound(...),  // 不存在即报错，不删
        _ => Database(...)
    })?;
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1 AND created_at >= ?2",
                 params![conversation_id, base_created])?;
    Ok(())
}
```

> 早期版本用 `COALESCE(..., '1970-01-01...')` 回退，message_id 缺失时会命中全部消息**删光整段对话**，已修复为存在性校验。

### Provider / 模型 / API Key / 插件 / 工具审计 / 设置

```sql
provider_configs(id, display_name, provider_type, api_base_url, is_enabled, config_json, created_at, updated_at)
models(id, provider_id, display_name, max_tokens, supports_vision, supports_tools, supports_streaming, pricing_json, PRIMARY KEY(id, provider_id))
api_keys(provider_id PRIMARY KEY, encrypted_key BLOB, nonce BLOB, created_at, updated_at)  -- AES-256-GCM（KeyManager 暂未接入）
plugins(id, name, version, description, author, wasm_path, manifest_json, config_json, is_enabled, installed_at, updated_at)
tool_audit_log(id, conversation_id, message_id, tool_name, tool_input, tool_output, status, duration_ms, created_at)
settings(key PRIMARY KEY, value TEXT, updated_at)  -- 键值设置（api_key/api_base_url/default_model/context_*）
```

### RAG 表

```sql
knowledge_bases(id, name, description, chunk_size, chunk_overlap, created_at, updated_at)
documents(id, kb_id, file_name, file_type, status, created_at)  -- status: pending|indexing|ready|error
chunks(id, doc_id, kb_id, chunk_index, content, embedding_json, metadata)  -- embedding_json: Vec<f64> JSON
chunks_fts USING fts5(content)  -- BM25 关键词检索，由 MIGRATION_005 触发器维护
```

详见 [rag.md](rag.md)。

## 迁移

版本化管理（`crates/conversation-store/src/migration.rs`），**每个迁移用 `unchecked_transaction` 包裹**，失败整体回滚：

```rust
const MIGRATIONS: &[(u32, &str)] = &[
    (1, MIGRATION_001_INITIAL),    // 核心表 + messages_fts + INSERT 触发器
    (2, MIGRATION_002_RAG),        // knowledge_bases/documents/chunks/chunks_fts（无触发器）
    (3, MIGRATION_003_AGENTS),     // agents 表
    (4, MIGRATION_004_AGENT_STYLE),// agents 加样式/参数列（ALTER TABLE）
    (5, MIGRATION_005_FTS_TRIGGERS),// 补 chunks_fts 触发器+回填，messages_fts DELETE/UPDATE 触发器
];

pub fn run_migrations(conn: &Connection) -> StoreResult<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY NOT NULL)")?;
    let current: u32 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0)).unwrap_or(0);
    for (version, sql) in MIGRATIONS {
        if *version > current {
            let tx = conn.unchecked_transaction()?;  // 事务包裹：失败回滚
            tx.execute_batch(sql)?;
            tx.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (?1)", [version])?;
            tx.commit()?;
        }
    }
    Ok(())
}
```

> 早期版本 `execute_batch` 无事务，多语句迁移（如 004 的 7 条 ALTER TABLE）中途失败会留下半应用状态，重试时首条 ALTER 报"列已存在"→ 数据库永久卡死。事务化后此问题修复。

### MIGRATION_005 要点

- `chunks_fts` 此前建表却无 INSERT 触发器，`hybrid_search` 的 FTS5 分支始终查空表 → 混合检索退化为纯向量。本迁移补 chunks 的 INSERT/DELETE/UPDATE 触发器并回填现有 chunks
- `messages_fts` 此前只有 INSERT 触发器，删/改消息后索引陈旧。本迁移补 DELETE/UPDATE 触发器
