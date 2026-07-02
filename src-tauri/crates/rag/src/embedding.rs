//! Embedding 客户端。调用 OpenAI 兼容的 embedding API。

use serde::Deserialize;
use std::time::Duration;

/// 嵌入向量 (f32 数组)
pub type Embedding = Vec<f32>;

/// 嵌入向量维度（Qwen/Qwen3-Embedding-8B）
pub const EMBEDDING_DIM: usize = 4096;

/// Embedding 客户端
pub struct EmbeddingClient {
    client: reqwest::Client,
    api_base_url: String,
    api_key: String,
    model: String,
}

impl EmbeddingClient {
    pub fn new(api_base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// 生成单个文本的嵌入向量
    pub async fn embed(&self, text: &str) -> Result<Embedding, String> {
        let mut result = self.embed_batch(&[text]).await?;
        result.pop().ok_or_else(|| "empty embedding response".to_string())
    }

    /// 批量生成嵌入向量（一次 API 调用）
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, String> {
        let resp = self.client
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

        let body: EmbeddingResponse = resp.json().await
            .map_err(|e| format!("embedding parse: {e}"))?;

        let embeddings: Vec<Embedding> = body.data.into_iter()
            .map(|d| d.embedding)
            .collect();

        if embeddings.len() != texts.len() {
            return Err(format!("expected {} embeddings, got {}", texts.len(), embeddings.len()));
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
