//! 流式相关共享类型。model-provider 产出、streaming 消费。

use serde::{Deserialize, Serialize};

use crate::types::ContentBlock;

/// 一次 chat 请求。跨 Provider 统一格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
    pub tools: Option<Vec<crate::types::ToolDefinition>>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,
    pub stop_sequences: Option<Vec<String>>,
}

/// 对话中的消息（传输用，区别于持久化的 Message）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant" | "tool"
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn user(text: &str) -> Self {
        Self {
            role: "user".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant(text: &str) -> Self {
        Self {
            role: "assistant".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }
}

/// 非流式响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub usage: UsageInfo,
    pub finish_reason: String,
}

/// 流式增量块。由 Provider 逐个产出。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamChunk {
    pub delta_text: Option<String>,
    pub delta_thinking: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageInfo>,
}

/// 工具调用增量（流式拼接用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// 在本次响应的工具调用列表中的索引
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    /// arguments 通常是分片到达的 JSON 字符串片段
    pub arguments_fragment: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
