//! ModelProvider trait：屏蔽各 Provider API 差异的统一抽象。

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use ripple_core::{ChatRequest, ChatResponse, ModelInfo, ProviderResult, StreamChunk, ToolDefinition};

/// 流式 chunk 的异步 Stream 类型
pub type ChunkStream =
    Pin<Box<dyn Stream<Item = ProviderResult<StreamChunk>> + Send>>;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Provider 唯一标识，如 "openai"
    fn provider_id(&self) -> &str;

    /// 显示名
    fn display_name(&self) -> &str;

    /// 列出可用模型（调用 /models 端点或返回预设）
    async fn list_models(&self, api_key: &str) -> ProviderResult<Vec<ModelInfo>>;

    /// 验证 API Key 是否有效
    async fn validate_api_key(&self, api_key: &str) -> ProviderResult<bool>;

    /// 非流式补全
    async fn chat(&self, api_key: &str, request: ChatRequest) -> ProviderResult<ChatResponse>;

    /// 流式补全，返回异步 Stream
    async fn chat_stream(&self, api_key: &str, request: ChatRequest) -> ProviderResult<ChunkStream>;

    /// 将统一 ToolDefinition 转为 Provider 特定的 JSON schema
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value;

    /// 是否支持工具调用
    fn supports_tools(&self) -> bool {
        true
    }

    /// 是否支持视觉输入
    fn supports_vision(&self) -> bool {
        false
    }
}
