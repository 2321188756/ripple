# crate: rag

检索增强生成。完全本地的知识库：文档摄入 → 向量化 → 混合检索 → 上下文注入。

## 职责

- 文档摄入：PDF/Markdown/TXT/代码 解析 + 清洗 + Markdown-aware 分块
- Embedding 生成：双轨制（云端 OpenAI / 本地 Ollama），批量调用
- 向量存储：sqlite-vec 虚拟表
- 全文索引：FTS5 BM25
- 混合检索：语义 KNN + BM25 + RRF 融合 + 可选重排序
- 上下文注入：`rag_search` 工具 + `@知识库` 自动注入

## 模块

```
src/
├── ingestion.rs       # 文档摄入管线
├── chunking.rs        # Chunker trait + Markdown/FixedSize/Sentence 策略
├── embedding.rs       # Embedding 客户端（云端/本地）
├── vector_store.rs    # sqlite-vec 读写
├── retrieval.rs       # 混合检索 + RRF + 重排序
├── injection.rs       # 上下文注入（System Prompt / User Message）
└── extractors/        # 各文件类型文本提取
    ├── pdf.rs
    ├── markdown.rs
    ├── txt.rs
    └── code.rs
```

详见 [docs/rag.md](../../../docs/rag.md)。
