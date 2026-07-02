# RAG 知识库设计

Ripple 内置检索增强生成（RAG），让 AI 能基于用户本地文档回答问题。设计目标：**完全本地、零外部服务、混合检索**。

## 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                      RAG Pipeline                             │
│                                                               │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────┐     │
│  │  Ingestion    │    │  Storage     │    │  Retrieval   │     │
│  │  文档摄入      │───→│  向量存储     │───→│  检索        │     │
│  │ - 文件导入     │    │ - sqlite-vec │    │ - 语义 KNN   │     │
│  │ - 解析清洗     │    │ - 元数据表    │    │ - BM25 关键词│     │
│  │ - 分块        │    │ - FTS5 索引   │    │ - RRF 融合   │     │
│  │ - Embedding   │    │              │    │ - 重排序      │     │
│  └──────────────┘    └──────────────┘    └──────┬──────┘     │
│                                                  │            │
│                                                  ▼            │
│  ┌────────────────────────────────────────────────────────┐  │
│  │            Context Injection（上下文注入）              │  │
│  │  检索 chunks → 去重 → 排序 → 注入 System/User 上下文     │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 向量库 | sqlite-vec | SQLite 原生扩展，与现有 DB 统一，无需独立服务 |
| Embedding | 双轨制：云端 OpenAI `text-embedding-3-small` / 本地 Ollama `nomic-embed-text` | 有 Key 用云端质量高，离线用本地 |
| 分块 | Markdown 感知 + 滑动窗口重叠 | 保持结构，512 token 块 + 50 重叠 |
| 检索 | 混合：语义向量 + FTS5 BM25 | 语义理解上下文，关键词精准匹配术语 |
| 融合 | Reciprocal Rank Fusion (RRF) | 无需分数归一化，鲁棒 |
| 重排序 | 可选 LLM/cross-encoder 重排 | 提升精度，按需开启 |

## 数据模型

```sql
-- 知识库
CREATE TABLE knowledge_bases (
    id                 TEXT PRIMARY KEY,
    name               TEXT NOT NULL,
    description        TEXT,
    embedding_provider TEXT,              -- "openai" | "ollama"
    embedding_model    TEXT,              -- "text-embedding-3-small" | "nomic-embed-text"
    embedding_dim      INTEGER,           -- 1536 / 768
    chunk_size         INTEGER DEFAULT 512,
    chunk_overlap      INTEGER DEFAULT 50,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

-- 文档
CREATE TABLE documents (
    id         TEXT PRIMARY KEY,
    kb_id      TEXT NOT NULL,
    file_path  TEXT,
    file_name  TEXT NOT NULL,
    file_type  TEXT,                      -- pdf, md, txt, code
    file_hash  TEXT,                      -- SHA256，去重/增量更新
    status     TEXT DEFAULT 'pending',    -- pending|indexing|ready|error
    error_msg  TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (kb_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
);

-- 文档块
CREATE TABLE chunks (
    id          TEXT PRIMARY KEY,
    doc_id      TEXT NOT NULL,
    kb_id       TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content     TEXT NOT NULL,
    token_count INTEGER,
    metadata    TEXT DEFAULT '{}',        -- JSON: 章节标题、页码
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

-- 全文索引（BM25 关键词检索）
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    chunk_id UNINDEXED,
    content,
    content='chunks',
    content_rowid='rowid'
);

-- 向量存储（sqlite-vec 虚拟表，维度按 embedding 模型定）
CREATE VIRTUAL TABLE chunk_embeddings USING vec0(
    chunk_id  TEXT PRIMARY KEY,
    embedding FLOAT[768]
);
```

## 摄入管线（Ingestion）

```
文件导入
  ↓
类型检测 → PDF(pdf-extract) / Markdown / TXT / 代码
  ↓
文本提取 + 清洗
  ↓
Markdown-aware Chunking:
  - 按 ## 标题优先切分
  - 超长块按段落再切
  - 相邻块 overlap 50 token
  - 记录元数据（源文件、章节、页码）
  ↓
Embedding 生成:
  - 批量调用（减少往返）
  - 本地模型限制并发（Ollama 默认并行 1）
  - 失败重试 3 次
  ↓
写入 chunks + chunk_embeddings + chunks_fts
  ↓
document.status = 'ready'
```

分块策略可插拔：

```rust
pub trait Chunker: Send + Sync {
    fn chunk(&self, text: &str, metadata: &HashMap<String, String>) -> Vec<Chunk>;
}

pub struct MarkdownChunker { config: ChunkingConfig }   // 默认
pub struct FixedSizeChunker { config: ChunkingConfig }
pub struct SentenceChunker { config: ChunkingConfig }
```

## 检索管线（Retrieval）

```
用户问题
  ↓
Embedding 生成（同摄入模型）
  ├─→ 语义检索: sqlite-vec KNN，Top-20
  └─→ 关键词检索: FTS5 BM25，Top-20
  ↓
RRF 融合: score = Σ 1/(k + rank_i)，k=60
  ↓
重排序（可选）: LLM 打分 / cross-encoder
  ↓
Top-5 chunks → 注入上下文
```

## 与对话集成

**方式 1：`rag_search` 内置工具**

AI 自主调用，适合"根据我的文档…"类提问：

```rust
ToolDefinition {
    name: "rag_search",
    description: "Search the user's knowledge base for relevant information",
    parameters: json!({
        "type": "object",
        "properties": {
            "query": {"type": "string"},
            "kb_id": {"type": "string", "description": "optional, searches all if omitted"},
            "top_k": {"type": "integer", "default": 5}
        },
        "required": ["query"]
    }),
}
```

**方式 2：`@知识库` 自动注入**

用户输入框用 `@知识库名` 引用，发送时后端自动检索并注入到 System Prompt：

```
用户: "@我的笔记 上周会议纪要说了什么？"
  → 解析 @引用 → 检索"我的笔记" → 注入 System Prompt → 发给 LLM
```

## 性能考量

| 场景 | 策略 |
|------|------|
| 大量文档索引 | 后台 Tokio task 异步，进度经 `kb:index-progress` 事件推送前端 |
| Embedding 批处理 | 单次 API 多文本，减少往返 |
| 向量检索 | sqlite-vec 10 万级 chunk KNN < 10ms |
| 本地 embedding | 首次 Ollama pull 慢，后续快；大索引建议云端 |
| 内存 | 向量不常驻内存，按需从 SQLite 读 |
| 增量更新 | file_hash 去重，未改动文件跳过 |
