# 模型抽象层与 Provider 实现

## 设计目标

用一个统一 trait 屏蔽各家 API 差异，让对话逻辑、工具调用、流式处理不感知具体 Provider。新增 Provider 只需实现 trait 并注册。

## 核心 Trait

```rust
// crates/model-provider/src/traits.rs
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Provider 唯一标识，如 "openai"
    fn provider_id(&self) -> &str;

    /// 显示名
    fn display_name(&self) -> &str;

    /// 列出可用模型（调用 /models 端点或返回预设）
    async fn list_models(&self, api_key: &str) -> Result<Vec<ModelInfo>, ProviderError>;

    /// 验证 API Key
    async fn validate_api_key(&self, api_key: &str) -> Result<bool, ProviderError>;

    /// 非流式补全
    async fn chat(&self, api_key: &str, request: ChatRequest)
        -> Result<ChatResponse, ProviderError>;

    /// 流式补全，返回异步 Stream
    async fn chat_stream(&self, api_key: &str, request: ChatRequest)
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;

    /// 将内部 ToolDefinition 转为 Provider 特定的 JSON schema
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value;

    /// 是否支持工具调用
    fn supports_tools(&self) -> bool { true }

    /// 是否支持视觉
    fn supports_vision(&self) -> bool { false }
}
```

## 共享类型

```rust
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,
    pub stop_sequences: Option<Vec<String>>,
}

pub struct ChatMessage {
    pub role: MessageRole,           // System | User | Assistant | Tool
    pub content: Vec<ContentBlock>,  // Text | Image | ToolCall | ToolResult | Thinking
}

pub struct StreamChunk {
    pub delta_text: Option<String>,
    pub delta_thinking: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageInfo>,
}
```

## Provider 注册表

```rust
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut r = Self { providers: HashMap::new() };
        r.register(Box::new(OpenAiProvider::new()));
        r.register(Box::new(AnthropicProvider::new()));
        r.register(Box::new(DeepSeekProvider::new()));   // 复用 OpenAI 兼容
        r.register(Box::new(OllamaProvider::new()));
        r.register(Box::new(GoogleProvider::new()));
        r.register(Box::new(OpenRouterProvider::new()));
        r
    }
}
```

## 内置 Provider

| Provider | API 格式 | 流式 | 工具调用 | 备注 |
|----------|----------|------|----------|------|
| OpenAI | OpenAI Chat | SSE | ✅ | 基准实现 |
| DeepSeek | OpenAI 兼容 | SSE | ✅ | 复用 OpenAiCompatibleProvider，换 base_url |
| OpenRouter | OpenAI 兼容 | SSE | ✅ | 同上，聚合多家模型 |
| Anthropic | Messages API | SSE（不同事件结构） | ✅（不同 schema） | 工具定义需转换 |
| Ollama | OpenAI 兼容（新版） | SSE | ✅ | 本地 `http://localhost:11434`，无 API Key |
| Google Gemini | Generative Language API | SSE | ✅ | API Key 在 query param |

### OpenAI 兼容复用

DeepSeek、OpenRouter、Ollama（新版）、自定义端点都遵循 OpenAI 格式，抽取一个 `OpenAiCompatibleProvider`，按配置注入不同 `base_url` 和模型列表：

```rust
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    provider_id: String,
    display_name: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(id: &str, name: &str, base_url: &str) -> Self { ... }
}

// 注册时
registry.register(Box::new(OpenAiCompatibleProvider::new(
    "deepseek", "DeepSeek", "https://api.deepseek.com/v1"
)));
```

### Anthropic 特殊处理

- 请求体：`messages` + 顶层 `system`（而非 system role 消息）
- 工具定义：`name` / `description` / `input_schema`（而非 `parameters`）
- 流式事件：`message_start` / `content_block_delta` / `message_stop`，需单独解析
- 思考链：`thinking` content block（Extended Thinking）

## 流式实现要点

```rust
async fn chat_stream(&self, api_key: &str, request: ChatRequest)
    -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>
{
    let response = self.client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&self.to_openai_request(&request))
        .send()
        .await?;

    // bytes_stream → 逐行解析 SSE → 映射为 StreamChunk
    let stream = response.bytes_stream()
        .map(|r| parse_sse_line(r?))
        .filter_map(|event| async move { event.map(|e| map_to_chunk(e)) });

    Ok(Box::pin(stream))
}
```

SSE 解析交给 `eventsource-stream`，业务层只做 `OpenAI chunk JSON → StreamChunk` 的映射。

## 自定义 Provider（用户自配）

设置页允许用户添加任意 OpenAI 兼容端点（如 Azure OpenAI、本地 vLLM、LM Studio）：

```typescript
invoke("provider_add", {
  config: {
    providerType: "custom_openai",
    displayName: "我的本地模型",
    apiBaseUrl: "http://localhost:8000/v1"
  },
  apiKey: "..."
})
```

后端用 `OpenAiCompatibleProvider` 实例化并注册。

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("Invalid API key")]
    InvalidApiKey,
    #[error("Rate limited")]
    RateLimited,
    #[error("Stream parse error: {0}")]
    StreamParse(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
}
```

错误映射为前端可读消息，并在 `chat:gen-error` 事件中携带 `errorCode` 供前端区分（如限流时显示重试按钮）。
