//! Schema 版本化管理。

use crate::StoreResult;

/// 所有迁移。按版本增序排列，只增不删。
const MIGRATIONS: &[(u32, &str)] = &[
    (1, MIGRATION_001_INITIAL),
    (2, MIGRATION_002_RAG),
    (3, MIGRATION_003_AGENTS),
    (4, MIGRATION_004_AGENT_STYLE),
    (5, MIGRATION_005_FTS_TRIGGERS),
    (6, MIGRATION_006_AGENT_MEMORY),
    (7, MIGRATION_007_MEMORY_TAGS),
    (8, MIGRATION_008_AGENT_PERMISSIONS),
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

const MIGRATION_004_AGENT_STYLE: &str = r#"
ALTER TABLE agents ADD COLUMN icon_color TEXT NOT NULL DEFAULT '#6366f1';
ALTER TABLE agents ADD COLUMN border_color TEXT NOT NULL DEFAULT '#6366f1';
ALTER TABLE agents ADD COLUMN border_width INTEGER NOT NULL DEFAULT 3;
ALTER TABLE agents ADD COLUMN name_color TEXT NOT NULL DEFAULT '#1e293b';
ALTER TABLE agents ADD COLUMN temperature REAL NOT NULL DEFAULT 0.7;
ALTER TABLE agents ADD COLUMN max_tokens INTEGER NOT NULL DEFAULT 4096;
ALTER TABLE agents ADD COLUMN top_p REAL NOT NULL DEFAULT 1.0;

INSERT OR IGNORE INTO schema_version (version) VALUES (4);
"#;

/// 005: 补齐 FTS5 触发器。
///  - `chunks_fts` 此前建表却无 INSERT 触发器，`hybrid_search` 的 FTS5 分支始终查空表，
///    导致"混合检索"退化为纯向量检索，关键词精确匹配完全失效。本迁移补齐
///    chunks 的 INSERT/DELETE/UPDATE 触发器，并回填现有 chunks。
///  - `messages_fts` 此前只有 INSERT 触发器，删/改消息后索引陈旧、rowid 复用会串内容，
///    这里补齐 DELETE/UPDATE 触发器。
const MIGRATION_005_FTS_TRIGGERS: &str = r#"
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    DELETE FROM chunks_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    DELETE FROM chunks_fts WHERE rowid = old.rowid;
    INSERT INTO chunks_fts(rowid, content) VALUES (new.rowid, new.content);
END;

-- 回填现有 chunks（此前 chunks_fts 为空，触发器只对新插入生效）
INSERT INTO chunks_fts(rowid, content) SELECT rowid, content FROM chunks;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    DELETE FROM messages_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    DELETE FROM messages_fts WHERE rowid = old.rowid;
    INSERT INTO messages_fts(rowid, conversation_id, content_text)
    VALUES (new.rowid, new.conversation_id, json_extract(new.content, '$[0].text'));
END;

INSERT OR IGNORE INTO schema_version (version) VALUES (5);
"#;

/// 006: Agent 记忆系统。每条记忆是一个 chunk（来自 dailynote/{agent}/ 下的文件），
/// 带 embedding_json 向量（cosine 语义检索）+ memories_fts 关键词索引。
/// 文件 hash 用于增量重建：文件变更时删旧 chunks 重新分块嵌入。
const MIGRATION_006_AGENT_MEMORY: &str = r#"
CREATE TABLE IF NOT EXISTS memories (
    id             TEXT PRIMARY KEY NOT NULL,
    agent_id       TEXT NOT NULL,
    file_path      TEXT NOT NULL,          -- 相对路径，如 dailynote/Aemeath/notes.md
    file_hash      TEXT NOT NULL,          -- SHA256，用于增量重建检测
    chunk_index    INTEGER NOT NULL,       -- 文件内块序号
    content        TEXT NOT NULL,          -- 块原文
    embedding_json TEXT,                   -- Vec<f32> JSON（NULL = 未嵌入）
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_agent ON memories(agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_file ON memories(file_path);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(content);

CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;
CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    DELETE FROM memories_fts WHERE rowid = old.rowid;
END;
CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
    DELETE FROM memories_fts WHERE rowid = old.rowid;
    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;

INSERT OR IGNORE INTO schema_version (version) VALUES (6);
"#;

/// 007: memories 表加 tags 列（关键词标签，用于不依赖 embedding 的高效 RAG 检索）。
/// 每条记忆 chunk 索引时自动提取 3-5 个关键词作为 tag，检索时提取 query 关键词匹配 tags。
const MIGRATION_007_MEMORY_TAGS: &str = r#"
ALTER TABLE memories ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';

INSERT OR IGNORE INTO schema_version (version) VALUES (7);
"#;

/// 008: Agent 工具权限。permission_level 三档：strict(每次审批) / elevated(可信任积累) / full(全放行)。
/// agent_trusted_tools 记录 elevated 模式下用户「信任此工具」的工具，后续自动放行，可收回。
const MIGRATION_008_AGENT_PERMISSIONS: &str = r#"
ALTER TABLE agents ADD COLUMN permission_level TEXT NOT NULL DEFAULT 'strict';

CREATE TABLE IF NOT EXISTS agent_trusted_tools (
    agent_id   TEXT NOT NULL,
    tool_name  TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (agent_id, tool_name)
);

INSERT OR IGNORE INTO schema_version (version) VALUES (8);
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
            // 每个迁移独立事务：中途失败整体回滚。否则多语句迁移（如 004 的 7 条
            // ALTER TABLE ADD COLUMN）部分提交后会留下半应用状态，重试时首条语句因
            // "列/表已存在"报错，数据库永久卡死、启动崩溃。
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| crate::StoreError::Database(e.to_string()))?;
            tx.execute_batch(sql)
                .map_err(|e| crate::StoreError::Migration {
                    version: *version,
                    details: e.to_string(),
                })?;
            tx.execute(
                "INSERT OR IGNORE INTO schema_version (version) VALUES (?1)",
                [version],
            )
            .map_err(|e| crate::StoreError::Database(e.to_string()))?;
            tx.commit()
                .map_err(|e| crate::StoreError::Database(e.to_string()))?;
        }
    }

    Ok(())
}
