//! ripple-model-provider: 模型抽象层 + 各 Provider 实现。
//!
//! 屏蔽云端 API 与本地模型差异，提供统一的 `ModelProvider` trait。

pub mod providers;
pub mod registry;
pub mod traits;

pub use providers::openai::OpenAiProvider;
pub use registry::ProviderRegistry;
pub use traits::{ChunkStream, ModelProvider};

use ripple_core::ProviderError;

/// reqwest 错误转 ProviderError。core 不依赖 reqwest，故转换放此处（显式函数，非 From trait，避免孤儿规则）。
pub(crate) fn map_reqwest_error(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Network(format!("timeout: {e}"))
    } else if e.is_connect() {
        ProviderError::Network(format!("connection: {e}"))
    } else {
        ProviderError::Network(e.to_string())
    }
}
