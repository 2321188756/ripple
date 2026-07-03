# RAG 知识库设计

Ripple 内置检索增强生成（RAG），让 AI 能基于用户本地文档回答问题。设计目标：**完全本地检索、零外部向量服务、混合检索**。

## 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                      RAG Pipeline                             │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────┐     │
│  │  Ingestion    │    │  Storage     │    │  Retrieval   │     │
│  │ - 文件导入     │───→│ - chunks 表   │───→│ - 向量 KNN   │     │
│  │ - 解析分块     │    │   embedding_  │    │   (Rust 余弦)│     │
│  │ - Embedding   │    │   json 列     │    │ - FTS5 BM25  │     │
│  │ - 批量嵌入     │    │ - chunks_fts │    │ - RRF 融合   │     │
│  └──────────────┘    └──────────────┘    └──────┬──────┘     │
│                                                  ▼            │
│            Context Injection（@kb 注入 / rag_search 工具）      │
└──────────────────────────────────────────────────────────────┘
```

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 向量存储 | `chunks.embedding_json` TEXT 列（JSON `Vec<f64>`） | 无需 sqlite-vec 扩展，与现有 DB 统一，简单可靠 |
| 相似度 | Rust 端 `cosine_similarity` 暴力计算 | 桌面级文档量（万级 chunk）足够快，无需 ANN 索引 |
| Embedding | 默认 newapi `Qwen3-Embedding-8B`，可配 | 经 OpenAI 兼容 `/embeddings` 端点，批量调用 |
| 分块 | Markdown 段落感知 + 滑动窗口重叠，按字符数 | 保持结构，默认 1000 字符块 + 100 重叠 |
| 检索 | 混合：向量余弦 + FTS5 BM25 | 语义理解上下文 + 关键词精准匹配术语 |
| 融合 | Reciprocal Rank Fusion (RRF)，用排名位置 | 无需分数归一化，鲁棒 |

## 数据模型

```sql
CREATE TABLE knowledge_bases (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT DEFAULT '',
    chunk_size INTEGER DEFAULT 1000, chunk_overlap INTEGER DEFAULT 100,
    created_at TEXT, updated_at TEXT
);

CREATE TABLE documents (
    id TEXT PRIMARY KEY, kb_id TEXT NOT NULL,
    file_name TEXT NOT NULL, file_type TEXT NOT NULL,
    status TEXT DEFAULT 'pending',  -- pending|indexing|ready|error
    created_at TEXT,
    FOREIGN KEY (kb_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
);

CREATE TABLE chunks (
    id TEXT PRIMARY KEY, doc_id TEXT NOT NULL, kb_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL, content TEXT NOT NULL,
    embedding_json TEXT,           -- Vec<f64> 序列化为 JSON（NULL = 未嵌入）
    metadata TEXT DEFAULT '{}',
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE chunks_fts USING fts5(content);  -- BM25 关键词检索
-- chunks_fts 的 INSERT/DELETE/UPDATE 触发器由 MIGRATION_005 补齐（早期版本缺失，导致 FTS5 检索失效）
```

> `delete_kb` 用事务删 chunks/documents/knowledge_bases，错误传播（早期版本 `.ok()` 吞错留孤儿行，已修）。

## 摄入管线（Ingestion）

```
文件导入
  ↓
类型检测 → TXT / Markdown / 代码（rs/py/js/ts）/ PDF
  ↓
文本提取 + 清洗
  ↓
chunk_text（Markdown 段落感知）:
  - 按 "\n\n" 拆段落，按字符数累积到 chunk_size
  - 超长段落按行再切
  - 相邻块 overlap（默认 100 字符）
  ↓
Embedding（EmbeddingClient，批量 10 条/批）:
  - embed_batch 调用 OpenAI 兼容 /embeddings
  - 批次失败 → 中止整篇文档，标 status='error'（不 continue 跳过）
  ↓
store_chunks_with_embeddings: chunks.iter().zip(embeddings) 写入
  ↓
document.status = 'ready'
```

> **关键容错**：`import_folder` 嵌入批次失败时必须 `break` 中止整篇文档。早期版本 `continue` 跳过失败批次，导致 `embedding_vec` 长度 < `chunks` 长度，`zip` 把后续向量错配到前面的 chunk，文档仍标 ready —— 静默数据损坏。已修。

```rust
// crates/rag/src/chunking.rs —— 按字符数（chars().count()）非字节
if current.chars().count() + para.chars().count() + 2 <= config.chunk_size { ... }
```

## 检索管线（Retrieval）

```rust
// crates/rag/src/store.rs::hybrid_search
// 1. 加载所有 chunks（embedding_json IS NOT NULL），按 kb_id 可选过滤（参数化 SQL）
// 2. 向量：cosine_similarity(query_emb, chunk_emb)，排序取 top_k*2
// 3. FTS5：SELECT ... FROM chunks_fts MATCH ? ORDER BY rank LIMIT ?2
//    记录每条结果的排名位置 pos (0,1,2,...)
// 4. RRF 融合：score = Σ 1/(k + pos_i)，k=60
//    —— 必须用排名位置，不能用原始 BM25 rank（负数 as usize 饱和为 0，破坏排序）
// 5. 取 top_k
```

> **RRF 修复**：FTS5 BM25 的 `rank` 是负数（越负越好）。早期版本存 `-(rank as f64)` 再 `(-fts_r) as usize`，负浮点转 usize 饱和为 0，所有 FTS 命中拿到相同 RRF 贡献，关键词排序信号丢失。改用结果排名位置后修复。

## 与对话集成

**方式 1：`rag_search` 内置工具**（AI 自主调用）

```rust
ToolDefinition { name: "rag_search", description: "Search knowledge base", ... }
// chat.rs::exec_rag_search：embed(query) → hybrid_search(None, 5) → 拼接结果文本
// 注意：embed 是 network 调用，不持有 DB 连接（避免连接池耗尽）
```

**方式 2：`@知识库名` 自动注入**

用户输入框用 `@知识库名` 引用，`inject_knowledge` 解析后检索并注入到 System Prompt：

```
用户: "@我的笔记 上周会议纪要说了什么？"
  → 解析 @引用 → 匹配 KB → embed(content) → hybrid_search(Some(kb_id), 3)
  → 注入 System Prompt → 发给 LLM
```

> `inject_knowledge` 同样先释放 DB 连接再 embed，检索时重新获取连接。

## 性能考量

| 场景 | 策略 |
|------|------|
| 大量文档索引 | `import_folder` 逐文件处理，单文件批次嵌入 |
| Embedding 批处理 | `embed_batch` 10 条/批，减少往返 |
| 向量检索 | 万级 chunk 余弦 < 50ms（暴力扫描） |
| 内存 | 向量按需从 SQLite 读，不常驻 |
| FTS5 | `chunks_fts` 触发器自动维护，BM25 高效 |
