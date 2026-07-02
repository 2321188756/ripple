//! 领域模型类型。与 SQLite schema、前端 TS 类型对齐。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 一次对话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub model_id: String,
    pub provider_id: String,
    pub system_prompt: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub metadata: serde_json::Value,
}

impl Conversation {
    pub fn new(provider_id: &str, model_id: &str, title: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.unwrap_or_else(|| "New Conversation".into()),
            created_at: now,
            updated_at: now,
            model_id: model_id.into(),
            provider_id: provider_id.into(),
            system_prompt: None,
            pinned: false,
            archived: false,
            metadata: serde_json::json!({}),
        }
    }
}

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 多模态内容块。一条消息可含多个块（文本、图片、工具调用、工具结果、思考链）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image { url: String, detail: Option<String> },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    Thinking { text: String },
}

impl ContentBlock {
    /// 取出纯文本（用于 FTS 索引、上下文摘要）
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// 一条消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
    pub created_at: DateTime<Utc>,
    pub token_count: Option<i32>,
    pub metadata: serde_json::Value,
}

impl Message {
    pub fn new_user(conversation_id: &str, text: &str) -> Self {
        Self::new(conversation_id, MessageRole::User, vec![ContentBlock::Text { text: text.into() }])
    }

    pub fn new_assistant(conversation_id: &str) -> Self {
        Self::new(conversation_id, MessageRole::Assistant, vec![])
    }

    pub fn new(conversation_id: &str, role: MessageRole, content: Vec<ContentBlock>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.into(),
            role,
            content,
            created_at: Utc::now(),
            token_count: None,
            metadata: serde_json::json!({}),
        }
    }

    /// 拼接所有文本块
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Provider 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    DeepSeek,
    Ollama,
    Google,
    OpenRouter,
    CustomOpenAI,
}

/// Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub display_name: String,
    pub provider_type: ProviderType,
    pub api_base_url: Option<String>,
    pub models: Vec<ModelInfo>,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
}

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub max_tokens: i32,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub pricing: Option<ModelPricing>,
}

impl ModelInfo {
    pub fn new(id: &str, display_name: &str) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            max_tokens: 4096,
            supports_vision: false,
            supports_tools: true,
            supports_streaming: true,
            pricing: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_1k: f64,
    pub output_per_1k: f64,
}

/// 工具来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Builtin,
    Plugin { plugin_id: String },
    UserDefined,
}

/// 工具定义（统一格式，由 model-provider 转换为各 Provider 的 schema）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema 描述参数
    pub parameters: serde_json::Value,
    pub source: ToolSource,
    /// 危险工具需用户审批后才执行
    pub requires_approval: bool,
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, parameters: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            source: ToolSource::Builtin,
            requires_approval: false,
        }
    }
}
