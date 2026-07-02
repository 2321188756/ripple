//! ripple-core: Ripple 的共享基础类型、错误与流式类型。
//!
//! 被所有业务 crate 依赖，本身不依赖任何业务 crate。

pub mod error;
pub mod stream;
pub mod types;

pub use error::{ProviderError, ProviderResult, RippleError, RippleResult};
pub use stream::{ChatMessage, ChatRequest, ChatResponse, StreamChunk, ToolCallDelta, UsageInfo};
pub use types::{
    ContentBlock, Conversation, Message, MessageRole, ModelInfo, ModelPricing, ProviderConfig,
    ProviderType, ToolDefinition, ToolSource,
};
