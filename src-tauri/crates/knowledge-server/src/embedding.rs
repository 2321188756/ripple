use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub struct EmbeddingVersion {
    pub base_url: String,
    pub model: String,
    pub expected_dimension: usize,
    pub batch_size: usize,
    pub request_timeout: Duration,
    pub max_retries: u32,
    pub secret: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingErrorKind {
    Retryable,
    RateLimited,
    Rejected,
    InvalidResponse,
}

#[derive(Debug, Clone)]
pub struct EmbeddingError {
    pub kind: EmbeddingErrorKind,
    pub retry_after: Option<Duration>,
}

pub struct OpenAiCompatibleEmbedding {
    client: reqwest::Client,
    version: EmbeddingVersion,
}

impl OpenAiCompatibleEmbedding {
    pub fn new(version: EmbeddingVersion) -> Result<Self, EmbeddingError> {
        if version.base_url.starts_with("http://") == false
            && version.base_url.starts_with("https://") == false
        {
            return Err(Self::invalid());
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| Self::invalid())?;
        Ok(Self { client, version })
    }

    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() || texts.len() > self.version.batch_size {
            return Err(Self::invalid());
        }
        let url = format!("{}/embeddings", self.version.base_url.trim_end_matches('/'));
        let mut attempt = 0u32;
        loop {
            let response = self
                .client
                .post(&url)
                .timeout(self.version.request_timeout)
                .bearer_auth(&self.version.secret)
                .json(&serde_json::json!({"model": self.version.model, "input": texts}))
                .send()
                .await;
            match response {
                Ok(response) if response.status().is_success() => {
                    let body = response
                        .json::<EmbeddingResponse>()
                        .await
                        .map_err(|_| Self::invalid())?;
                    return validate_response(body, texts.len(), self.version.expected_dimension);
                }
                Ok(response) => {
                    let status = response.status();
                    let retry_after = parse_retry_after(response.headers().get("retry-after"));
                    let kind = if status == StatusCode::TOO_MANY_REQUESTS {
                        EmbeddingErrorKind::RateLimited
                    } else if matches!(
                        status.as_u16(),
                        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504
                    ) {
                        EmbeddingErrorKind::Retryable
                    } else {
                        EmbeddingErrorKind::Rejected
                    };
                    if matches!(kind, EmbeddingErrorKind::Rejected)
                        || attempt >= self.version.max_retries
                    {
                        return Err(EmbeddingError { kind, retry_after });
                    }
                    sleep_before_retry(attempt, retry_after).await;
                    attempt += 1;
                }
                Err(_) => {
                    if attempt >= self.version.max_retries {
                        return Err(Self::retryable());
                    }
                    sleep_before_retry(attempt, None).await;
                    attempt += 1;
                }
            }
        }
    }

    fn invalid() -> EmbeddingError {
        EmbeddingError {
            kind: EmbeddingErrorKind::InvalidResponse,
            retry_after: None,
        }
    }
    fn retryable() -> EmbeddingError {
        EmbeddingError {
            kind: EmbeddingErrorKind::Retryable,
            retry_after: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    index: usize,
    embedding: Vec<f32>,
}

fn validate_response(
    body: EmbeddingResponse,
    expected_count: usize,
    expected_dimension: usize,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    if body.data.len() != expected_count {
        return Err(OpenAiCompatibleEmbedding::invalid());
    }
    let mut ordered = vec![None; expected_count];
    for item in body.data {
        if item.index >= expected_count
            || ordered[item.index].is_some()
            || item.embedding.len() != expected_dimension
            || item.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(OpenAiCompatibleEmbedding::invalid());
        }
        ordered[item.index] = Some(item.embedding);
    }
    ordered
        .into_iter()
        .map(|item| item.ok_or_else(OpenAiCompatibleEmbedding::invalid))
        .collect()
}

fn parse_retry_after(value: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    value?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .map(|seconds| Duration::from_secs(seconds.min(300)))
}

async fn sleep_before_retry(attempt: u32, retry_after: Option<Duration>) {
    let fallback =
        Duration::from_millis(100u64.saturating_mul(2u64.saturating_pow(attempt.min(8))));
    tokio::time::sleep(
        retry_after
            .unwrap_or(fallback)
            .min(Duration::from_secs(300)),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorders_and_validates_embedding_response() {
        let body = EmbeddingResponse {
            data: vec![
                EmbeddingItem {
                    index: 1,
                    embedding: vec![2.0, 3.0],
                },
                EmbeddingItem {
                    index: 0,
                    embedding: vec![0.0, 1.0],
                },
            ],
        };
        let result = validate_response(body, 2, 2).unwrap();
        assert_eq!(result, vec![vec![0.0, 1.0], vec![2.0, 3.0]]);
    }

    #[test]
    fn rejects_duplicate_or_non_finite_vectors() {
        let duplicate = EmbeddingResponse {
            data: vec![
                EmbeddingItem {
                    index: 0,
                    embedding: vec![1.0],
                },
                EmbeddingItem {
                    index: 0,
                    embedding: vec![2.0],
                },
            ],
        };
        assert!(validate_response(duplicate, 2, 1).is_err());
        let non_finite = EmbeddingResponse {
            data: vec![EmbeddingItem {
                index: 0,
                embedding: vec![f32::NAN],
            }],
        };
        assert!(validate_response(non_finite, 1, 1).is_err());
    }
}
