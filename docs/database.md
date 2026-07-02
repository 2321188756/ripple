# 数据库设计

SQLite 单文件存储所有数据。启用 WAL 模式 + FTS5 全文搜索 + sqlite-vec 向量扩展。

## 初始化

```rust
// crates/conversation-store/src/db.rs
pub fn init_db(path: &Path) -> Result<DbPool> {
    let pool = r2d2::Pool::new(r2d2_sqlite::SqliteConnectionManager::file(path))?;
    let conn = pool.get()?;
    conn.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        PRAGMA synchronous = NORMAL;
    ")?;
    run_migrations(&conn)?;
    Ok(pool)
}
```

## Schema

### 对话

```sql
CREATE TABLE conversations (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL DEFAULT 'New Conversation',
    created_at    TEXT NOT NULL,           -- ISO 8601
    updated_at    TEXT NOT NULL,
    model_id      TEXT NOT NULL,
    provider_id   TEXT NOT NULL,
    system_prompt TEXT,
    pinned        INTEGER NOT NULL DEFAULT 0,
    archived      INTEGER NOT NULL DEFAULT 0,
    metadata      TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_conv_updated ON conversations(updated_at DESC);
CREATE INDEX idx_conv_pinned  ON conversations(pinned) WHERE pinned = 1;
CREATE INDEX idx_conv_title   ON conversations(title);
```

### 消息

```sql
CREATE TABLE messages (
    id              TEXT PRIMARY KEY,
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

-- 全文搜索
CREATE VIRTUAL TABLE messages_fts USING fts5(
    conversation_id UNINDEXED,
    content_text,
    content='messages',
    content_rowid='rowid'
);

-- 同步触发器
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, conversation_id, content_text)
    VALUES (new.rowid, new.conversation_id, json_extract(new.content, '$[0].text'));
END;
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, conversation_id, content_text)
    VALUES ('delete', old.rowid, old.conversation_id, '');
END;
CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, conversation_id, content_text)
    VALUES ('delete', old.rowid, old.conversation_id, '');
    INSERT INTO messages_fts(rowid, conversation_id, content_text)
    VALUES (new.rowid, new.conversation_id, json_extract(new.content, '$[0].text'));
END;
```

### Provider 与模型

```sql
CREATE TABLE provider_configs (
    id            TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL,
    provider_type TEXT NOT NULL CHECK(provider_type IN (
        'openai','anthropic','deepseek','ollama','google','openrouter','custom_openai'
    )),
    api_base_url  TEXT,
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    config_json   TEXT NOT NULL DEFAULT '{}',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE models (
    id                 TEXT NOT NULL,
    provider_id        TEXT NOT NULL,
    display_name       TEXT NOT NULL,
    max_tokens         INTEGER NOT NULL DEFAULT 4096,
    supports_vision    INTEGER NOT NULL DEFAULT 0,
    supports_tools     INTEGER NOT NULL DEFAULT 0,
    supports_streaming INTEGER NOT NULL DEFAULT 1,
    pricing_json       TEXT,
    PRIMARY KEY (id, provider_id),
    FOREIGN KEY (provider_id) REFERENCES provider_configs(id) ON DELETE CASCADE
);
```

### API Key（加密存储）

```sql
CREATE TABLE api_keys (
    provider_id    TEXT PRIMARY KEY,
    encrypted_key  BLOB NOT NULL,    -- AES-256-GCM 密文
    nonce          BLOB NOT NULL,    -- AES-GCM nonce
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES provider_configs(id) ON DELETE CASCADE
);
```

### 插件

```sql
CREATE TABLE plugins (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    version       TEXT NOT NULL,
    description   TEXT,
    author        TEXT,
    wasm_path     TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    config_json   TEXT DEFAULT '{}',
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    installed_at  TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
```

### 工具审计

```sql
CREATE TABLE tool_audit_log (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    message_id      TEXT NOT NULL,
    tool_name       TEXT NOT NULL,
    tool_input      TEXT NOT NULL,
    tool_output     TEXT,
    status          TEXT NOT NULL CHECK(status IN ('approved','denied','error','success')),
    duration_ms     INTEGER,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
```

### 设置

```sql
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,     -- JSON
    updated_at TEXT NOT NULL
);
```

### RAG 表

见 [rag.md](rag.md)：`knowledge_bases` / `documents` / `chunks` / `chunks_fts` / `chunk_embeddings`。

## 迁移

版本化管理，启动时自动执行：

```rust
// crates/conversation-store/src/migration.rs
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/001_initial.sql")),
    (2, include_str!("../migrations/002_rag.sql")),
    // ...
];

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER)", [])?;
    let current: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0)
    )?;
    for (v, sql) in MIGRATIONS {
        if *v > current {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO schema_version VALUES (?)", [v])?;
        }
    }
    Ok(())
}
```

迁移文件放 `crates/conversation-store/migrations/`，每个文件一个原子 schema 变更，只增不改（破坏性变更走新版本 + 数据搬运）。
