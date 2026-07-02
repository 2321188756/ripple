//! Schema 版本化管理。

use crate::StoreResult;

/// 所有迁移。按版本增序排列，只增不删。
const MIGRATIONS: &[(u32, &str)] = &[
    (1, MIGRATION_001_INITIAL),
    (2, MIGRATION_002_RAG),
    (3, MIGRATION_003_AGENTS),
];

/// 001: 初始 schema —— 核心业务表
const MIGRATION_001_INITIAL: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
    id            TEXT PRIMARY KEY NOT NULL,
    title         TEXT NOT NULL DEFAULT 'New Conversation',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    model_id      TEXT NOT NULL,
    provider_id   TEXT NOT NULL,
    system_prompt TEXT,
    pinned        INTEGER NOT NULL DEFAULT 0,
    archived      INTEGER NOT NULL DEFAULT 0,
    metadata      TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_conv_updated ON conversations(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_conv_pinned ON conversations(pinned) WHERE pinned = 1;

CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    role            TEXT NOT NULL CHECK(role IN ('system','user','assistant','tool')),
    content         TEXT NOT NULL,
    summary         TEXT,
    created_at      TEXT NOT NULL,
    token_count     INTEGER,
    metadata        TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conversation_id, created_at);

-- 不依赖外部表内容（避免 content='messages' 的集成复杂度）
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    conversation_id UNINDEXED,
    content_text
);

CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, conversation_id, content_text)
    VALUES (new.rowid, new.conversation_id, json_extract(new.content, '$[0].text'));
END;

-- DELETE/UPDATE 触发器暂缺：FTS5 delete 命令要求 exact value 校验，
-- 在 JSON content 提取场景下容易因格式差异失败。
-- 聊天以插入为主，对删/改后的搜索精度要求不高。后续可加重建逻辑。

CREATE TABLE IF NOT EXISTS provider_configs (
    id            TEXT PRIMARY KEY NOT NULL,
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

CREATE TABLE IF NOT EXISTS models (
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

CREATE TABLE IF NOT EXISTS api_keys (
    provider_id    TEXT PRIMARY KEY NOT NULL,
    encrypted_key  BLOB NOT NULL,
    nonce          BLOB NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES provider_configs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS plugins (
    id            TEXT PRIMARY KEY NOT NULL,
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

CREATE TABLE IF NOT EXISTS tool_audit_log (
    id              TEXT PRIMARY KEY NOT NULL,
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

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY NOT NULL
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);
"#;

/// 002: RAG 知识库表
const MIGRATION_002_RAG: &str = r#"
CREATE TABLE IF NOT EXISTS knowledge_bases (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    description  TEXT DEFAULT '',
    chunk_size   INTEGER DEFAULT 1000,
    chunk_overlap INTEGER DEFAULT 100,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    id         TEXT PRIMARY KEY NOT NULL,
    kb_id      TEXT NOT NULL,
    file_name  TEXT NOT NULL,
    file_type  TEXT NOT NULL,
    status     TEXT DEFAULT 'pending',
    created_at TEXT NOT NULL,
    FOREIGN KEY (kb_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS chunks (
    id              TEXT PRIMARY KEY NOT NULL,
    doc_id          TEXT NOT NULL,
    kb_id           TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    content         TEXT NOT NULL,
    embedding_json  TEXT,
    metadata        TEXT DEFAULT '{}',
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content
);

INSERT OR IGNORE INTO schema_version (version) VALUES (2);
"#;

/// 003: 自定义 Agent 表
const MIGRATION_003_AGENTS: &str = r#"
CREATE TABLE IF NOT EXISTS agents (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    description  TEXT DEFAULT '',
    system_prompt TEXT NOT NULL DEFAULT 'You are a helpful assistant.',
    tools         TEXT DEFAULT '[]',
    model         TEXT DEFAULT '',
    icon          TEXT DEFAULT '🤖',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);
"#;

/// 运行所有待执行的迁移。
pub fn run_migrations(conn: &rusqlite::Connection) -> StoreResult<()> {
    // 确保版本表存在（migration v1 也会创建它，但可能连 v1 都还没跑）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY NOT NULL);",
    )
    .map_err(|e| crate::StoreError::Database(e.to_string()))?;

    let current: u32 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .unwrap_or(0);

    for (version, sql) in MIGRATIONS {
        if *version > current {
            conn.execute_batch(sql)
                .map_err(|e| crate::StoreError::Migration {
                    version: *version,
                    details: e.to_string(),
                })?;
            conn.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (?1)", [version])
                .map_err(|e| crate::StoreError::Database(e.to_string()))?;
        }
    }

    Ok(())
}
