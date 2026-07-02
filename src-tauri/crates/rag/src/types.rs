//! RAG 共享类型

use serde::{Deserialize, Serialize};

/// 知识库
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub created_at: String,
}

/// 文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub kb_id: String,
    pub file_name: String,
    pub file_type: String,
    pub status: String, // pending | indexing | ready | error
    pub created_at: String,
}

/// 文档块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub kb_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub metadata: serde_json::Value,
}

/// 检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub content: String,
    pub score: f64,
    pub kb_id: String,
    pub doc_name: String,
    pub metadata: serde_json::Value,
}

/// 检索请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub kb_id: Option<String>,
    pub top_k: usize,
}
