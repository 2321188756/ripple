//! ripple-rag: 检索增强生成。

pub mod chunking;
pub mod embedding;
pub mod store;
pub mod types;

pub use chunking::{chunk_text, ChunkConfig};
pub use embedding::{cosine_similarity, EmbeddingClient};
pub use types::{Chunk, Document, KnowledgeBase, SearchResult};
