//! Embedding 客户端。调用 OpenAI 兼容的 embedding API。

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::types::Chunk;

const DEFAULT_BATCH_SIZE: usize = 10;
const DEFAULT_BATCH_TIMEOUT: Duration = Duration::from_secs(30);

/// 嵌入向量 (f32 数组)
pub type Embedding = Vec<f32>;

/// 嵌入向量维度（Qwen/Qwen3-Embedding-8B）
pub const EMBEDDING_DIM: usize = 4096;

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, String>;

    async fn embed(&self, text: &str) -> Result<Embedding, String> {
        let mut result = self.embed_batch(&[text]).await?;
        result
            .pop()
            .ok_or_else(|| "empty embedding response".to_string())
    }
}

/// 按固定批次生成所有 chunk 的嵌入；任一批次失败或超时即中止，不返回部分结果。
pub async fn embed_chunks<P: EmbeddingProvider + ?Sized>(
    provider: &P,
    chunks: &[Chunk],
) -> Result<Vec<Embedding>, String> {
    embed_chunks_with_options(provider, chunks, DEFAULT_BATCH_SIZE, DEFAULT_BATCH_TIMEOUT).await
}

async fn embed_chunks_with_options<P: EmbeddingProvider + ?Sized>(
    provider: &P,
    chunks: &[Chunk],
    batch_size: usize,
    timeout: Duration,
) -> Result<Vec<Embedding>, String> {
    if chunks.is_empty() {
        return Err("document must contain at least one chunk".into());
    }
    if batch_size == 0 {
        return Err("embedding batch size must be non-zero".into());
    }

    let mut all = Vec::with_capacity(chunks.len());
    for batch in chunks.chunks(batch_size) {
        let texts: Vec<&str> = batch.iter().map(|chunk| chunk.content.as_str()).collect();
        let embeddings = tokio::time::timeout(timeout, provider.embed_batch(&texts))
            .await
            .map_err(|_| format!("embedding request timed out after {}s", timeout.as_secs()))??;
        if embeddings.len() != batch.len() {
            return Err(format!(
                "expected {} embeddings, got {}",
                batch.len(),
                embeddings.len()
            ));
        }
        all.extend(embeddings);
    }
    Ok(all)
}

/// Embedding 客户端
pub struct EmbeddingClient {
    client: reqwest::Client,
    api_base_url: String,
    api_key: String,
    model: String,
}

impl EmbeddingClient {
    pub fn with_client(
        client: reqwest::Client,
        api_base_url: &str,
        api_key: &str,
        model: &str,
    ) -> Self {
        Self {
            client,
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    pub fn new(api_base_url: &str, api_key: &str, model: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("build reqwest client: {e}"))?;
        Ok(Self {
            client,
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        })
    }

    /// 生成单个文本的嵌入向量
    pub async fn embed(&self, text: &str) -> Result<Embedding, String> {
        EmbeddingProvider::embed(self, text).await
    }

    /// 批量生成嵌入向量（一次 API 调用）
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, String> {
        EmbeddingProvider::embed_batch(self, texts).await
    }
}

#[async_trait]
impl EmbeddingProvider for EmbeddingClient {
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, String> {
        let resp = self
            .client
            .post(format!("{}/embeddings", self.api_base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts,
            }))
            .send()
            .await
            .map_err(|e| format!("embedding request: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("embedding API error: {}", resp.status()));
        }

        let body: EmbeddingResponse = resp
            .json()
            .await
            .map_err(|e| format!("embedding parse: {e}"))?;

        let embeddings: Vec<Embedding> = body.data.into_iter().map(|d| d.embedding).collect();

        if embeddings.len() != texts.len() {
            return Err(format!(
                "expected {} embeddings, got {}",
                texts.len(),
                embeddings.len()
            ));
        }

        Ok(embeddings)
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Embedding,
}

/// 余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum();
    let nb: f32 = b.iter().map(|x| x * x).sum();
    (dot as f64) / ((na * nb).sqrt() as f64 + 1e-10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FailingProvider {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl EmbeddingProvider for FailingProvider {
        async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, String> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 1 {
                Err("second batch failed".into())
            } else {
                Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
            }
        }
    }

    fn chunks(count: usize) -> Vec<Chunk> {
        (0..count)
            .map(|index| Chunk {
                id: format!("chunk-{index}"),
                doc_id: "doc".into(),
                kb_id: "kb".into(),
                chunk_index: index,
                content: format!("content-{index}"),
                metadata: serde_json::json!({}),
            })
            .collect()
    }

    #[tokio::test]
    async fn multi_batch_failure_never_returns_partial_embeddings() {
        let provider = FailingProvider {
            calls: AtomicUsize::new(0),
        };
        let error = embed_chunks_with_options(&provider, &chunks(3), 2, Duration::from_secs(1))
            .await
            .unwrap_err();

        assert_eq!(error, "second batch failed");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }

    struct SlowProvider;

    #[async_trait]
    impl EmbeddingProvider for SlowProvider {
        async fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Embedding>, String> {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(vec![vec![1.0]])
        }
    }

    #[tokio::test]
    async fn batch_timeout_is_explicit() {
        let error =
            embed_chunks_with_options(&SlowProvider, &chunks(1), 10, Duration::from_millis(1))
                .await
                .unwrap_err();

        assert!(error.contains("timed out"));
    }
}
