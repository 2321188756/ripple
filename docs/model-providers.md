# 模型抽象层与 Provider 实现

## 设计目标

用一个统一 trait 屏蔽各家 API 差异，让对话逻辑、工具调用、流式处理不感知具体 Provider。新增 Provider 只需实现 trait 并注册。

> **当前实现**：仅 `OpenAiProvider`（OpenAI 兼容）。所有模型经 newapi 之类的 OpenAI 兼容端点接入。Anthropic / Google / Ollama 等为规划。

## 核心 Trait

```rust
// crates/model-provider/src/traits.rs
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn display_name(&self) -> &str;
    async fn chat(&self, api_key: &str, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
    async fn chat_stream(&self, api_key: &str, request: ChatRequest)
        -> Result<Pin<Box<dyn Stream<Item = ProviderResult<StreamChunk>> + Send>>, ProviderError>;
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value;
    // ...
}
```

## 共享类型（`crates/core/src/types.rs`）

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

pub enum ContentBlock {
    Text { text: String },
    Image { url: String, detail: Option<String> },
    ToolCall { id: String, name: String, arguments: serde_json::Value },
    ToolResult { tool_call_id: String, content: String },
    Thinking { text: String },
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
// crates/model-provider/src/registry.rs
pub struct ProviderRegistry { providers: HashMap<String, Arc<dyn ModelProvider>> }
impl ProviderRegistry {
    pub fn with_builtins() -> Self { /* 注册 OpenAiProvider 等 */ }
}
```

`commands::chat::do_chat_stream_inner` 中按需用 `OpenAiProvider::new_dynamic("newapi", "newapi", &base_url)` 实例化（当前每请求新建，规划：复用 `reqwest::Client` 存 AppState）。

## OpenAiProvider 实现

| 能力 | 实现 |
|------|------|
| 流式 | `chat_stream` POST `/chat/completions` (stream=true)，`eventsource-stream` 解析 SSE → `StreamChunk` |
| 非流式 | `chat` POST `/chat/completions`，`map_non_stream_response` 映射 |
| 工具调用 | 请求体带 `tools`；流式累积 `tool_calls` delta；非流式解析 `tool_calls` |
| 鉴权 | `Authorization: Bearer <api_key>` |

### 非流式响应映射（`map_non_stream_response`）

同时返回 Text 块与 ToolCall 块：

```rust
let mut blocks = Vec::new();
if !c.message.content.is_empty() {
    blocks.push(ContentBlock::Text { text: c.message.content.clone() });
}
for tc in &c.message.tool_calls {
    blocks.push(ContentBlock::ToolCall { id: tc.id.clone(), name: tc.function.name.clone(), arguments: ... });
}
```

> 早期版本在 `content` 非空时走 else 分支只产出 Text，丢弃 tool_calls。但 OpenAI 常同时返回文本内容与工具调用，导致非流式工具调用丢失。已修。

### 流式实现要点

```rust
async fn chat_stream(&self, api_key, request) -> Result<...> {
    let response = self.client.post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&self.to_openai_request(&request))
        .send().await?;
    let stream = response.bytes_stream()
        .map(|r| parse_sse_line(r?))
        .filter_map(|event| async move { event.map(|e| map_to_chunk(e)) });
    Ok(Box::pin(stream))
}
```

## 工具调用循环（`chat_with_tools`）

```rust
const MAX_TOOL_ROUNDS: u32 = 8;  // 防止模型反复 tool call 死循环
let mut had_error = false;
loop {
    if cancelled.load(SeqCst) { break; }              // 锁存取消兜底
    if iterations >= MAX_TOOL_ROUNDS { break; }
    iterations += 1;
    let stream = provider.chat_stream(api_key, request.clone()).await?;
    let mut aborted = false;
    tokio::select! {
        _ = consume_stream(stream, |ev| match ev {
            Text(t) => { collected_text.push_str(&t); emit("chat:stream-chunk", ...); }
            Signal(chunk) => { /* 累积 tool_calls / finish_reason */ }
            Error(e) => { had_error = true; }          // 不静默，设标志
            End => {}
        }) => {}
        _ = cancel.notified() => { aborted = true; }  // stop_generation 中断
    }
    if aborted || had_error || tool_calls.is_empty() { break; }
    // 执行工具 → emit("chat:tool-call") → 回填 tool 结果 → 继续循环
}
if had_error {
    emit("chat:gen-error", ...);                        // 通知前端，不静默截断
    return Ok(collected_text);                          // 保留部分文本（spawn 落库）
}
emit("chat:gen-complete", ...);
Ok(collected_text)
```

## 取消机制

`stop_generation` 从 `active_streams` 取出该会话的 `ActiveStream`，`cancelled.store(true)` + `cancel.notify_waiters()`。`chat_with_tools` 的 `select!` 收到 notify 后丢弃 `consume_stream` future（HTTP 流中断），返回已累积的部分文本。

## 错误处理

```rust
pub enum ProviderError {
    Network(String),
    Api { status: u16, body: String },  // 如 400/401/429
    InvalidApiKey,
    RateLimited,
    StreamParse(String),
    ...
}
```

错误经 `chat:gen-error` 事件 `{ conversation_id, message_id, error }` 推前端；前端 `handleStreamError` 保留已生成部分为助手消息、清流、显示错误横幅。

> 流式 400 错误（如模型不支持 `image_url`）经此路径提示用户，不再静默截断当成功。
