//! ripple-rag: 检索增强生成。

pub mod chunking;
pub mod embedding;
pub mod file_read;
pub mod store;
pub mod types;

pub use chunking::{chunk_text, ChunkConfig};
pub use embedding::{cosine_similarity, embed_chunks, EmbeddingClient, EmbeddingProvider};
pub use file_read::read_file_content;
pub use types::{Chunk, Document, KnowledgeBase, SearchResult};
