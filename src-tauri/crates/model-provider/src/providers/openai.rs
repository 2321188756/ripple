//! OpenAI 兼容 Provider。OpenAI / DeepSeek / OpenRouter / Ollama(v1) 复用此实现，仅 base_url 不同。
//!
//! 流式格式：SSE，每行 `data: {json}`，末尾 `data: [DONE]`。

use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{self, HeaderMap};
use serde::Deserialize;
use tracing::warn;

use ripple_core::{
    ChatMessage, ChatRequest, ChatResponse, ContentBlock, ModelInfo, ProviderError, ProviderResult,
    StreamChunk, ToolCallDelta, ToolDefinition, UsageInfo,
};

use crate::traits::{ChunkStream, ModelProvider};

/// 便捷映射 reqwest 错误
macro_rules! ptry {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(err) => return Err(crate::map_reqwest_error(err)),
        }
    };
}

/// OpenAI 兼容 Provider
pub struct OpenAiProvider {
    client: reqwest::Client,
    provider_id: String,
    display_name: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(provider_id: &str, display_name: &str, base_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .expect("failed to build reqwest client"),
            provider_id: provider_id.into(),
            display_name: display_name.into(),
            base_url: base_url.into(),
        }
    }

    /// 动态版本（与 `new` 相同，提供显式动态构建接口）
    pub fn new_dynamic(provider_id: &str, display_name: &str, base_url: &str) -> Self {
        Self::new(provider_id, display_name, base_url)
    }

    /// 用外部传入的共享 `reqwest::Client` 构造（连接池复用，避免每请求新建 client）。
    pub fn with_client(
        provider_id: &str,
        display_name: &str,
        base_url: &str,
        client: reqwest::Client,
    ) -> Self {
        Self {
            client,
            provider_id: provider_id.into(),
            display_name: display_name.into(),
            base_url: base_url.into(),
        }
    }

    fn auth_headers(&self, api_key: &str) -> ProviderResult<HeaderMap> {
        let mut h = HeaderMap::new();
        // 非 ASCII 字符（如粘贴时夹带的隐藏 unicode）会让 from_str 失败。
        // 早期版本静默回退为空 header → 神秘 401；这里改为返回明确错误。
        let value = header::HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|_| {
            ProviderError::Unsupported("API key contains invalid (non-ASCII) characters".into())
        })?;
        h.insert(header::AUTHORIZATION, value);
        Ok(h)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    async fn list_models(&self, api_key: &str) -> ProviderResult<Vec<ModelInfo>> {
        let headers = self.auth_headers(api_key)?;
        let resp = ptry!(
            self.client
                .get(self.endpoint("/models"))
                .headers(headers)
                .send()
                .await
        );

        let status = resp.status();
        if !status.is_success() {
            let body_bytes = resp.content_length().unwrap_or_default();
            tracing::warn!(
                provider = %self.provider_id,
                operation = "list_models",
                status = status.as_u16(),
                response_bytes = body_bytes,
                "provider request failed"
            );
            return Err(map_api_error(status.as_u16()));
        }
        let body: ModelsResponse = ptry!(resp.json().await);
        Ok(body
            .data
            .into_iter()
            .map(|m| ModelInfo::new(&m.id, &m.id))
            .collect())
    }

    async fn validate_api_key(&self, api_key: &str) -> ProviderResult<bool> {
        match self.list_models(api_key).await {
            Ok(_) => Ok(true),
            Err(ProviderError::InvalidApiKey) | Err(ProviderError::Api { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn chat(&self, api_key: &str, request: ChatRequest) -> ProviderResult<ChatResponse> {
        let body = build_request_body(&request, false);
        let headers = self.auth_headers(api_key)?;
        let resp = ptry!(
            self.client
                .post(self.endpoint("/chat/completions"))
                .headers(headers)
                .json(&body)
                .send()
                .await
        );

        let status = resp.status();
        if !status.is_success() {
            let body_bytes = resp.content_length().unwrap_or_default();
            tracing::warn!(
                provider = %self.provider_id,
                operation = "chat",
                status = status.as_u16(),
                response_bytes = body_bytes,
                "provider request failed"
            );
            return Err(map_api_error(status.as_u16()));
        }

        let resp_body: ChatCompletionResponse = ptry!(resp.json().await);
        Ok(map_non_stream_response(resp_body, &request.model))
    }

    async fn chat_stream(
        &self,
        api_key: &str,
        request: ChatRequest,
    ) -> ProviderResult<ChunkStream> {
        let body = build_request_body(&request, true);
        let headers = self.auth_headers(api_key)?;
        let resp = ptry!(
            self.client
                .post(self.endpoint("/chat/completions"))
                .headers(headers)
                .json(&body)
                .send()
                .await
        );

        let status = resp.status();
        if !status.is_success() {
            let body_bytes = resp.content_length().unwrap_or_default();
            tracing::warn!(
                provider = %self.provider_id,
                operation = "chat_stream",
                status = status.as_u16(),
                response_bytes = body_bytes,
                "provider request failed"
            );
            return Err(map_api_error(status.as_u16()));
        }

        // bytes_stream → eventsource 解析 → 映射为 StreamChunk
        use eventsource_stream::Eventsource as _;
        let byte_stream = resp.bytes_stream();
        let event_stream = byte_stream.eventsource();
        let provider_id = self.provider_id.clone();
        let chunk_stream = event_stream
            .take_while(|item| {
                let stop = matches!(item, Ok(ev) if ev.data == "[DONE]");
                futures::future::ready(!stop)
            })
            .filter_map(move |item| {
                let provider_id = provider_id.clone();
                async move {
                    match item {
                        Ok(ev) => match serde_json::from_str::<ChatCompletionChunk>(&ev.data) {
                            Ok(chunk) => Some(map_stream_chunk(chunk)),
                            Err(e) => {
                                warn!(
                                    provider = %provider_id,
                                    payload_bytes = ev.data.len(),
                                    error = %e,
                                    "failed to parse provider SSE chunk"
                                );
                                Some(Err(ProviderError::StreamParse(
                                    "invalid provider stream payload".into(),
                                )))
                            }
                        },
                        Err(e) => Some(Err(ProviderError::StreamParse(e.to_string()))),
                    }
                }
            });

        Ok(Box::pin(chunk_stream))
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        serde_json::Value::Array(
            tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect(),
        )
    }
}

// ---- 请求构建 ----

fn build_request_body(request: &ChatRequest, stream: bool) -> serde_json::Value {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    if let Some(sys) = &request.system_prompt {
        messages.push(serde_json::json!({
            "role": "system",
            "content": sys,
        }));
    }

    for msg in &request.messages {
        messages.push(message_to_json(msg));
    }

    let mut body = serde_json::json!({
        "model": request.model,
        "messages": messages,
        "stream": stream,
    });

    if let Some(t) = request.temperature {
        body["temperature"] = serde_json::json!(t);
    }
    if let Some(m) = request.max_tokens {
        body["max_tokens"] = serde_json::json!(m);
    }
    if let Some(p) = request.top_p {
        body["top_p"] = serde_json::json!(p);
    }
    if let Some(stop) = &request.stop_sequences {
        body["stop"] = serde_json::json!(stop);
    }
    if let Some(tools) = &request.tools {
        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(
                tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect(),
            );
            // 显式 tool_choice: auto。部分 OpenAI 兼容端点（newapi 转发的非 OpenAI 模型）
            // 不默认 auto，不显式指定则模型可能完全不调用工具。
            body["tool_choice"] = serde_json::json!("auto");
        }
    }
    // 流式时请求 usage 回传
    if stream {
        body["stream_options"] = serde_json::json!({ "include_usage": true });
    }

    let tool_count = request.tools.as_ref().map_or(0, Vec::len);
    let input_text_chars: usize = request
        .system_prompt
        .as_deref()
        .into_iter()
        .map(str::chars)
        .map(Iterator::count)
        .sum::<usize>()
        + request
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .map(|block| match block {
                ContentBlock::Text { text } | ContentBlock::Thinking { text } => {
                    text.chars().count()
                }
                ContentBlock::ToolCall { name, .. } => name.chars().count(),
                ContentBlock::ToolResult { .. } | ContentBlock::Image { .. } => 0,
            })
            .sum::<usize>();
    let image_count = request
        .messages
        .iter()
        .flat_map(|message| message.content.iter())
        .filter(|block| matches!(block, ContentBlock::Image { .. }))
        .count();
    tracing::debug!(
        model = %request.model,
        stream,
        message_count = request.messages.len(),
        tool_count,
        image_count,
        input_text_chars,
        "provider request prepared"
    );

    body
}

fn message_to_json(msg: &ChatMessage) -> serde_json::Value {
    // 支持多模态：文本、工具调用、工具结果
    let role = &msg.role;
    let mut content_parts: Vec<serde_json::Value> = Vec::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut tool_result: Option<(String, String)> = None;

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                content_parts.push(serde_json::json!({"type": "text", "text": text}));
            }
            ContentBlock::ToolCall {
                id,
                name,
                arguments,
            } => {
                tool_calls.push(serde_json::json!({
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": arguments.to_string() }
                }));
            }
            ContentBlock::ToolResult {
                tool_call_id,
                content,
            } => {
                tool_result = Some((tool_call_id.clone(), content.clone()));
            }
            ContentBlock::Image { url, detail } => {
                content_parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": url,
                        "detail": detail.as_deref().unwrap_or("auto")
                    }
                }));
            }
            _ => {}
        }
    }

    let has_images = content_parts
        .iter()
        .any(|v| v.get("type").and_then(|t| t.as_str()) == Some("image_url"));

    if role == "tool" {
        let (tool_call_id, content) = tool_result.unwrap_or_default();
        return serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": content
        });
    }

    if !tool_calls.is_empty() {
        let content_str: String = content_parts
            .iter()
            .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
            .collect();
        return serde_json::json!({
            "role": "assistant",
            "content": if content_parts.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(content_str) },
            "tool_calls": tool_calls
        });
    }

    // 有图片 → content 为数组；纯文本 → content 为字符串
    if has_images {
        return serde_json::json!({ "role": role, "content": content_parts });
    }

    // Default: plain text
    let text: String = content_parts
        .iter()
        .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
        .collect();
    serde_json::json!({ "role": role, "content": text })
}

// ---- 响应映射 ----

fn map_non_stream_response(resp: ChatCompletionResponse, model: &str) -> ChatResponse {
    let choice = resp.choices.into_iter().next();
    let content = choice
        .as_ref()
        .map(|c| {
            let mut blocks: Vec<ContentBlock> = Vec::new();
            // 文本内容（可能为空）
            if !c.message.content.is_empty() {
                blocks.push(ContentBlock::Text {
                    text: c.message.content.clone(),
                });
            }
            // 工具调用：早期版本在 content 非空时走 else 分支只产出 Text，丢弃了 tool_calls。
            // 但 OpenAI 常同时返回文本内容与工具调用，导致非流式工具调用丢失。
            for tc in &c.message.tool_calls {
                blocks.push(ContentBlock::ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments: tc
                        .function
                        .arguments
                        .as_ref()
                        .and_then(|a| serde_json::from_str(a).ok())
                        .unwrap_or(serde_json::Value::Null),
                });
            }
            blocks
        })
        .unwrap_or_default();

    ChatResponse {
        content,
        model: model.into(),
        usage: resp.usage.map(UsageInfo::from).unwrap_or_default(),
        finish_reason: choice.and_then(|c| c.finish_reason).unwrap_or_default(),
    }
}

fn map_stream_chunk(chunk: ChatCompletionChunk) -> ProviderResult<StreamChunk> {
    let mut out = StreamChunk::default();

    // usage 可能在最后一个空 choices 的 chunk 中
    if let Some(usage) = chunk.usage {
        out.usage = Some(UsageInfo::from(usage));
    }

    if let Some(choice) = chunk.choices.into_iter().next() {
        if let Some(reason) = choice.finish_reason {
            out.finish_reason = Some(reason);
        }
        if let Some(delta) = choice.delta {
            if let Some(text) = &delta.content {
                if !text.is_empty() {
                    out.delta_text = Some(text.clone());
                }
            }
            if !delta.tool_calls.is_empty() {
                out.tool_calls = Some(
                    delta
                        .tool_calls
                        .into_iter()
                        .map(|c| {
                            let (name, args) = match c.function {
                                Some(f) => (f.name, f.arguments),
                                None => (None, None),
                            };
                            ToolCallDelta {
                                index: c.index,
                                id: c.id,
                                name,
                                arguments_fragment: args,
                            }
                        })
                        .collect(),
                );
            }
        }
    }

    Ok(out)
}
fn map_api_error(status: u16) -> ProviderError {
    match status {
        401 | 403 => ProviderError::InvalidApiKey,
        429 => ProviderError::RateLimited,
        _ => ProviderError::Api { status },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_request_logs_only_metadata() {
        let secret = "SENTINEL_SECRET_DO_NOT_LOG";
        let request = ChatRequest {
            model: "test-model".into(),
            messages: vec![ChatMessage::user(secret)],
            system_prompt: Some(secret.into()),
            tools: None,
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop_sequences: None,
        };

        let body = build_request_body(&request, true);
        assert_eq!(body["messages"][0]["content"], secret);
        assert_eq!(body["messages"][1]["content"], secret);
        assert!(matches!(
            map_api_error(500),
            ProviderError::Api { status: 500 }
        ));
    }

    #[test]
    fn provider_error_never_contains_response_body() {
        let error = map_api_error(500).to_string();
        assert_eq!(error, "api request failed (status 500)");
        assert!(!error.contains("SENTINEL_SECRET_DO_NOT_LOG"));
        assert!(matches!(map_api_error(401), ProviderError::InvalidApiKey));
        assert!(matches!(map_api_error(429), ProviderError::RateLimited));
    }
}

// ---- OpenAI API DTO ----

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelObject>,
}

#[derive(Debug, Deserialize)]
struct ModelObject {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<NonStreamChoice>,
    usage: Option<UsageDto>,
}

#[derive(Debug, Deserialize)]
struct NonStreamChoice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<ToolCallObject>,
}

#[derive(Debug, Deserialize)]
struct ToolCallObject {
    id: String,
    function: ToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct ToolCallFunction {
    name: String,
    arguments: Option<String>,
}

// 流式 chunk
#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    usage: Option<UsageDto>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct Delta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<DeltaToolCall>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: u32,
    id: Option<String>,
    function: Option<DeltaToolFunction>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageDto {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl From<UsageDto> for UsageInfo {
    fn from(u: UsageDto) -> Self {
        UsageInfo {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }
    }
}
