//! 统一错误类型。各业务 crate 的错误可 `?` 转换为 `RippleError`。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RippleError {
    #[error("database error: {0}")]
    Database(String),

    #[error("provider error: {0}")]
    Provider(#[from] crate::ProviderError),

    #[error("security error: {0}")]
    Security(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Provider 层错误。model-provider crate 也会重新导出此类型。
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network error: {0}")]
    Network(String),

    #[error("api error (status {status}): {body}")]
    Api { status: u16, body: String },

    #[error("invalid api key")]
    InvalidApiKey,

    #[error("rate limited")]
    RateLimited,

    #[error("stream parse error: {0}")]
    StreamParse(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("unsupported feature: {0}")]
    Unsupported(String),
}

pub type RippleResult<T> = Result<T, RippleError>;
pub type ProviderResult<T> = Result<T, ProviderError>;
