//! 聊天相关命令：发送消息、停止生成。

use std::time::Duration;

use ripple_core::{ChatMessage, ChatRequest, ContentBlock, Message, MessageRole, ProviderError, UsageInfo};
use ripple_model_provider::{ModelProvider, OpenAiProvider};
use ripple_streaming::consume_stream;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::warn;

use crate::state::AppState;
use crate::commands::conversation;

/// 带超时的数据库连接获取
macro_rules! db_conn {
    ($state:expr) => {
        match $state.db.get_timeout(Duration::from_secs(5)) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "db connection timeout");
                return Err(format!("db timeout: {e}"));
            }
        }
    };
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    api_key: String,
    api_base_url: Option<String>,
    model: Option<String>,
    agent_mode: Option<bool>,
    images: Option<Vec<String>>,
) -> Result<String, String> {
    tracing::info!(%conversation_id, len = content.len(), "send_message start");

    // 1. 获取对话
    let conn = db_conn!(state);
    let conversation = match conversation::get_conversation_inner(&conn, &conversation_id) {
        Ok(c) => c,
        Err(e) => { tracing::error!(%e, "conversation not found"); return Err(format!("conversation not found: {e}")); }
    };
    drop(conn);
    tracing::info!(model = %conversation.model_id, provider = %conversation.provider_id, "conversation loaded");

    // 2. 保存用户消息到 DB（含图片）
    let mut blocks = vec![ContentBlock::Text { text: content.clone() }];
    if let Some(imgs) = &images {
        for url in imgs {
            blocks.push(ContentBlock::Image { url: url.clone(), detail: Some("auto".into()) });
        }
    }
    let user_msg = Message::new(&conversation_id, MessageRole::User, blocks);
    { let conn = db_conn!(state); let r = ripple_conversation_store::MessageRepo::insert(&conn, &user_msg);
        if let Err(e) = r { tracing::warn!(error = %e, "save user msg failed"); } }

    // 3. 加载历史
    let conn = db_conn!(state);
    let history = ripple_conversation_store::MessageRepo::list_by_conversation(&conn, &conversation_id, 50, None).unwrap_or_default();
    drop(conn);

    // 4. 知识库注入 + 设置
    let kb_inject = inject_knowledge(&content, &state, &api_key).await;
    let ctx_enabled = get_setting_bool(&state, "context_enabled").await.unwrap_or(true);
    let ctx_window = get_setting_int(&state, "context_recent_window").await.unwrap_or(20) as usize;
    let ctx_interval = get_setting_int(&state, "context_summary_interval").await.unwrap_or(10) as usize;
    let ctx_max_tokens = get_setting_int(&state, "context_max_tokens").await.unwrap_or(32000) as usize;

    // 5. 启动流（共享逻辑）
    let (message_id, _) = do_chat_stream_inner(
        app.clone(),
        state.active_streams.clone(),
        state.db.clone(),
        &conversation, &history, &content,
        &api_key, &api_base_url, &model, agent_mode, kb_inject,
        ctx_enabled, ctx_window, ctx_interval, ctx_max_tokens,
    ).await?;

    Ok(message_id)
}

/// send_message 和 regenerate 共享的核心逻辑：构建请求 + 启动流
#[allow(clippy::too_many_arguments)]
async fn do_chat_stream_inner(
    app: AppHandle,
    active_streams: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::state::ActiveStream>>>,
    db: ripple_conversation_store::DbPool,
    conversation: &ripple_core::Conversation,
    history: &[ripple_core::Message],
    content: &str,
    api_key: &str,
    api_base_url: &Option<String>,
    model: &Option<String>,
    agent_mode: Option<bool>,
    kb_inject_text: Option<String>,
    setting_ctx_enabled: bool,
    setting_ctx_window: usize,
    setting_ctx_interval: usize,
    setting_ctx_max_tokens: usize,
) -> Result<(String, String), String> {
    use ripple_core::{ChatMessage, ChatRequest, ContentBlock, Message, MessageRole};
    use ripple_model_provider::OpenAiProvider;

    let mut chat_messages: Vec<ChatMessage> = history.iter().map(|m| {
        let role = match m.role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::Tool => "tool" }.into();
        // 保留所有 content blocks（包括图片）
        let content: Vec<ContentBlock> = m.content.iter().filter_map(|block| match block {
            ContentBlock::Text { .. } => Some(block.clone()),
            ContentBlock::Image { .. } => Some(block.clone()),
            ContentBlock::ToolCall { .. } => None, // tool calls handled separately
            ContentBlock::ToolResult { .. } => None,
            ContentBlock::Thinking { .. } => None,
        }).collect();
        ChatMessage { role, content: if content.is_empty() { vec![ContentBlock::Text { text: m.text() }] } else { content } }
    }).collect();

    let agent_mode = agent_mode.unwrap_or(false);
    let mut sys_text = resolve_agent_placeholders(
        conversation.system_prompt.as_deref().unwrap_or("You are a helpful AI assistant.")
    );
    if let Some(inject) = kb_inject_text { sys_text = format!("{sys_text}\n\n--- Knowledge base ---\n{inject}\n---"); }
    let sys_text = if agent_mode {
        format!("{sys_text}\n\nYou are now in AGENT mode...\nIf a tool fails, try an alternative approach.")
    } else { sys_text };

    if setting_ctx_enabled && history.len() > 10 {
        let config = ripple_context::ContextBuilderConfig {
            recent_window: setting_ctx_window.max(5),
            summary_interval: setting_ctx_interval.max(2),
            budget: ripple_context::BudgetRatio::default(),
        };
        let counter = std::sync::Arc::new(ripple_context::CharApproxCounter);
        let summarizer = std::sync::Arc::new(ripple_context::TemplateSummarizer::default());
        let builder = ripple_context::ContextBuilder::new(config, counter, summarizer);
        let ctx = builder.assemble(Some(&sys_text), history, setting_ctx_max_tokens.max(4000), 4096).await;
        chat_messages = ctx.messages.iter().map(|m| ChatMessage { role: m.role.clone(), content: m.content.clone() }).collect();
        chat_messages.push(ChatMessage::user(content));
        if ctx.truncated { tracing::info!(tokens = ctx.total_tokens, summaries = ctx.summary_count, "context truncated"); }
    } else {
        chat_messages.insert(0, ChatMessage { role: "system".into(), content: vec![ContentBlock::Text { text: sys_text }] });
        chat_messages.push(ChatMessage::user(content));
    }

    let model_id = model.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if conversation.model_id == "default" { "deepseek-v4-flash".into() } else { conversation.model_id.clone() }
    });
    let base_url = api_base_url.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let provider = OpenAiProvider::new_dynamic("newapi", "newapi", &base_url);

    let chat_request = ChatRequest {
        model: model_id.clone(),
        messages: chat_messages,
        system_prompt: None,
        tools: Some(crate::commands::tools::builtin_tools()),
        temperature: Some(0.7),
        max_tokens: Some(4096),
        top_p: None,
        stop_sequences: None,
    };
    tracing::info!(%base_url, "request built (do_chat_stream_inner)");

    let message_id = uuid::Uuid::new_v4().to_string();
    let conv_id = conversation.id.clone();

    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    active_streams.lock().await.insert(conv_id.clone(), crate::state::ActiveStream {
        conversation_id: conv_id.clone(),
        cancel: cancel.clone(),
        cancelled: cancelled.clone(),
    });

    let spawn_msg_id = message_id.clone();
    let conv_id2 = conv_id.clone();
    let api_key_clone = api_key.to_string();
    let db2 = db.clone();
    let streams2 = active_streams.clone();
    let cancel2 = cancel.clone();
    let cancelled2 = cancelled.clone();

    tokio::spawn(async move {
        tracing::info!("spawned stream task (do_chat_stream_inner)");
        let result = chat_with_tools(&app, &provider, &api_key_clone, chat_request, &conv_id2, &spawn_msg_id, &model_id, &db2, &cancel2, &cancelled2).await;

        streams2.lock().await.remove(&conv_id2);

        if let Ok(text) = &result {
            if !text.is_empty() {
                let msg = Message {
                    id: spawn_msg_id.clone(),
                    conversation_id: conv_id2.clone(),
                    role: MessageRole::Assistant,
                    content: vec![ContentBlock::Text { text: text.clone() }],
                    created_at: chrono::Utc::now(), token_count: None,
                    metadata: serde_json::json!({}),
                };
                if let Ok(conn) = db2.get_timeout(Duration::from_secs(3)) {
                    if let Err(e) = ripple_conversation_store::MessageRepo::insert(&conn, &msg) {
                        tracing::warn!(error = %e, "save assistant msg failed");
                    }
                }
            }
        }
        if let Err(e) = &result {
            tracing::error!(error = %e, "stream error");
            let _ = app.emit("chat:gen-error", GenErrorPayload { conversation_id: conv_id2, message_id: spawn_msg_id, error: e.to_string() });
        }
    });

    Ok((message_id, content.to_string()))
}

/// 停止生成
#[tauri::command]
pub async fn stop_generation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), String> {
    // 取出该会话的流句柄并触发取消。锁存标志 + notify 双保险：
    // 即便流的 select! 尚未首次 poll，下一轮循环顶也会读到 cancelled=true。
    let stream = state.active_streams.lock().await.remove(&conversation_id);
    if let Some(s) = stream {
        s.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        s.cancel.notify_waiters();
    }
    Ok(())
}

/// 重生成：删除指定消息之后的内容，重新运行生成流程
#[tauri::command]
pub async fn regenerate(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: String,
    api_key: String,
    api_base_url: Option<String>,
    model: Option<String>,
    agent_mode: Option<bool>,
) -> Result<String, String> {
    tracing::info!(%conversation_id, %message_id, "regenerate start");

    // 1. 获取对话
    let conn = db_conn!(state);
    let conversation = match conversation::get_conversation_inner(&conn, &conversation_id) {
        Ok(c) => c,
        Err(e) => return Err(format!("conversation not found: {e}")),
    };
    drop(conn);

    // 2. 删除 message_id 之后的所有消息（含本身）
    //    delete_from 在 message_id 不存在时返回 NotFound —— 必须在此报错，
    //    否则后续会基于未截断的历史重生成，且掩盖前端传错 id 的问题。
    {
        let conn = db_conn!(state);
        if let Err(e) = ripple_conversation_store::MessageRepo::delete_from(&conn, &conversation_id, &message_id) {
            tracing::warn!(error = %e, "regenerate delete_from failed");
            return Err(format!("regenerate: cannot locate message {message_id}: {e}"));
        }
    }

    // 3. 加载剩余历史
    let conn = db_conn!(state);
    let history = ripple_conversation_store::MessageRepo::list_by_conversation(&conn, &conversation_id, 50, None).unwrap_or_default();
    drop(conn);

    // 4. 获取最后一条 user 消息的内容
    let last_user_content = history.iter()
        .rev()
        .find(|m| m.role == MessageRole::User)
        .map(|m| m.text())
        .unwrap_or_default();
    if last_user_content.is_empty() {
        return Err("No user message found to regenerate from".into());
    }
    let content = last_user_content;

    // 5. 知识库注入 + 设置
    let kb_inject = inject_knowledge(&content, &state, &api_key).await;
    let ctx_enabled = get_setting_bool(&state, "context_enabled").await.unwrap_or(true);
    let ctx_window = get_setting_int(&state, "context_recent_window").await.unwrap_or(20) as usize;
    let ctx_interval = get_setting_int(&state, "context_summary_interval").await.unwrap_or(10) as usize;
    let ctx_max_tokens = get_setting_int(&state, "context_max_tokens").await.unwrap_or(32000) as usize;

    // 6. 复用核心流逻辑
    let (msg_id, _) = do_chat_stream_inner(
        app.clone(),
        state.active_streams.clone(),
        state.db.clone(),
        &conversation, &history, &content,
        &api_key, &api_base_url, &model, agent_mode, kb_inject,
        ctx_enabled, ctx_window, ctx_interval, ctx_max_tokens,
    ).await?;

    Ok(msg_id)
}

// ---- 事件载荷 ----

#[derive(Clone, Serialize)]
pub struct StreamChunkPayload {
    pub conversation_id: String,
    pub message_id: String,
    pub delta_text: Option<String>,
    pub finish_reason: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct GenCompletePayload {
    pub conversation_id: String,
    pub message_id: String,
    pub usage: UsageInfo,
}
#[derive(Clone, Serialize)]
pub struct GenErrorPayload {
    pub conversation_id: String,
    pub message_id: String,
    pub error: String,
}

/// 工具调用循环
async fn chat_with_tools(
    app: &AppHandle,
    provider: &OpenAiProvider,
    api_key: &str,
    mut request: ChatRequest,
    conversation_id: &str,
    message_id: &str,
    model: &str,
    db: &ripple_conversation_store::DbPool,
    cancel: &std::sync::Arc<tokio::sync::Notify>,
    cancelled: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    use tauri::Emitter;
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;

    let mut collected_text = String::new();
    let mut tool_calls: HashMap<u32, ToolCallAccum> = HashMap::new();
    let mut had_error = false;
    const MAX_TOOL_ROUNDS: u32 = 8;
    let mut iterations = 0u32;

    loop {
        // 锁存取消检查：stop_generation 在 select! 首次 poll 前触发时的兜底
        if cancelled.load(Ordering::SeqCst) {
            tracing::info!("chat_with_tools cancelled before round");
            break;
        }
        if iterations >= MAX_TOOL_ROUNDS {
            tracing::warn!(iterations, "tool loop hit max rounds, stopping");
            break;
        }
        iterations += 1;

        let mut local_text = String::new();
        tool_calls.clear();

        let stream = provider.chat_stream(api_key, request.clone()).await.map_err(|e| e.to_string())?;

        // 消费流；与取消信号竞速，stop 时立即中断 HTTP 流
        let mut aborted = false;
        tokio::select! {
            _ = consume_stream(stream, |event| {
                match event {
                    ripple_streaming::StreamEvent::Text(text) => {
                        local_text.push_str(&text);
                        collected_text.push_str(&text);
                        let _ = app.emit("chat:stream-chunk", StreamChunkPayload {
                            conversation_id: conversation_id.to_string(),
                            message_id: message_id.to_string(),
                            delta_text: Some(text), finish_reason: None,
                        });
                    }
                    ripple_streaming::StreamEvent::Signal(chunk) => {
                        if let Some(calls) = chunk.tool_calls {
                            for tc in calls {
                                let e = tool_calls.entry(tc.index).or_insert_with(|| ToolCallAccum { id: None, name: None, arguments: String::new() });
                                if let Some(id) = tc.id { e.id = Some(id); }
                                if let Some(name) = tc.name { e.name = Some(name); }
                                if let Some(frag) = tc.arguments_fragment { e.arguments.push_str(&frag); }
                            }
                        }
                        if let Some(reason) = chunk.finish_reason {
                            let _ = app.emit("chat:stream-chunk", StreamChunkPayload {
                                conversation_id: conversation_id.to_string(),
                                message_id: message_id.to_string(),
                                delta_text: None, finish_reason: Some(reason),
                            });
                        }
                    }
                    ripple_streaming::StreamEvent::Error(e) => {
                        warn!(error = %e, "stream error");
                        had_error = true;
                    }
                    ripple_streaming::StreamEvent::End => {}
                }
            }) => {}
            _ = cancel.notified() => { aborted = true; }
        }

        if aborted {
            tracing::info!("chat_with_tools aborted by stop");
            break;
        }
        if had_error {
            break;
        }

        if tool_calls.is_empty() { break; }

        tracing::info!(count = tool_calls.len(), "executing tool calls");

        // Assistant 消息（含 tool_call）
        let mut blocks: Vec<ContentBlock> = Vec::new();
        if !local_text.is_empty() { blocks.push(ContentBlock::Text { text: local_text }); }
        for (_, tc) in &tool_calls {
            blocks.push(ContentBlock::ToolCall {
                id: tc.id.clone().unwrap_or_default(),
                name: tc.name.clone().unwrap_or_default(),
                arguments: serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null),
            });
        }
        request.messages.push(ChatMessage { role: "assistant".into(), content: blocks });
        request.model = model.to_string();

        // 执行工具 + 推结果 + 发事件
        for (_, tc) in &tool_calls {
            let name = tc.name.as_deref().unwrap_or("unknown");
            let args: serde_json::Value = serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
            let exec = match name {
                "calculator" => crate::commands::tools::exec_calculator(&args),
                "rag_search" => exec_rag_search(db, api_key, &args).await,
                other if other.starts_with("plugin_") => {
                    crate::commands::plugins::exec_by_tool_name(other, &args)
                }
                other => Err(format!("unknown tool: {other}")),
            };
            let (status, output): (String, String) = match &exec {
                Ok(v) => ("success".to_string(), v.clone()),
                Err(e) => ("error".to_string(), e.clone()),
            };

            tracing::info!(tool = %name, status = %status, "tool executed");

            let tid = tc.id.clone().unwrap_or_default();
            request.messages.push(ChatMessage {
                role: "tool".into(),
                content: vec![ContentBlock::ToolResult { tool_call_id: tid, content: output.clone() }],
            });

            let payload = serde_json::json!({ "tool_name": name, "tool_input": tc.arguments, "tool_output": output, "status": status });
            let _ = app.emit("chat:tool-call", payload);
        }
    }

    // 流错误：通知前端，并返回已收集的部分文本（由 spawn 落库），避免静默截断当成功
    if had_error {
        let _ = app.emit("chat:gen-error", GenErrorPayload {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            error: "stream error".into(),
        });
        return Ok(collected_text);
    }

    // 正常完成或被 stop 中断：发 gen-complete，前端据此 finalize 已显示的部分文本
    let _ = app.emit("chat:gen-complete", GenCompletePayload {
        conversation_id: conversation_id.to_string(),
        message_id: message_id.to_string(),
        usage: UsageInfo::default(),
    });

    Ok(collected_text)
}

/// 执行 RAG 搜索
async fn exec_rag_search(
    db: &ripple_conversation_store::DbPool,
    api_key: &str,
    args: &serde_json::Value,
) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let _kb_id = args.get("kb_id").and_then(|v| v.as_str());
    if query.is_empty() { return Err("missing query".into()); }

    let client = ripple_rag::EmbeddingClient::new(
        "http://192.168.0.123:3000/v1", api_key, "Qwen/Qwen3-Embedding-8B",
    );
    // 先获取嵌入（网络调用，可能耗时数秒），不持有 DB 连接，避免连接池耗尽
    let emb = client.embed(&query).await?;
    let conn = db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let results = ripple_rag::store::hybrid_search(&conn, &emb, &query, None, 5)?;

    if results.is_empty() {
        return Ok("No relevant information found in the knowledge base.".into());
    }

    let mut out = String::from("Knowledge base results:\n\n");
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!("[{i}] [doc: {}] (score: {:.3})\n{}\n\n", r.doc_name, r.score, r.content));
    }
    Ok(out)
}

/// 从用户消息中检测 @知识库 并检索注入
async fn inject_knowledge(content: &str, state: &State<'_, AppState>, api_key: &str) -> Option<String> {
    // 查找 @xxx 模式
    let content_chars: Vec<char> = content.chars().collect();
    let mut kb_names = Vec::new();
    let mut i = 0;
    while i < content_chars.len() {
        if content_chars[i] == '@' && i + 1 < content_chars.len() && content_chars[i + 1].is_alphabetic() {
            let start = i + 1;
            let mut end = start;
            while end < content_chars.len() && content_chars[end].is_alphanumeric() {
                end += 1;
            }
            if end > start { kb_names.push(content_chars[start..end].iter().collect::<String>()); }
            i = end;
        } else { i += 1; }
    }
    if kb_names.is_empty() { return None; }

    // 先查所有匹配的 KB（用完即释放连接，避免跨 embed 网络调用长期持有）
    let kb_ids: Vec<(String, String)> = {
        let conn = state.db.get_timeout(std::time::Duration::from_secs(5)).ok()?;
        let mut kb_ids = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT id, name FROM knowledge_bases") {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))) {
                for row in rows.flatten() {
                    for name in &kb_names {
                        if row.1.contains(name) { kb_ids.push((row.0.clone(), row.1.clone())); break; }
                    }
                }
            }
        }
        kb_ids
    };
    if kb_ids.is_empty() { return None; }

    // 嵌入查询（网络调用），不持有 DB 连接
    let client = ripple_rag::EmbeddingClient::new("http://192.168.0.123:3000/v1", api_key, "Qwen/Qwen3-Embedding-8B");
    let emb = client.embed(content).await.ok()?;

    // 重新获取连接做检索
    let conn = state.db.get_timeout(std::time::Duration::from_secs(5)).ok()?;
    let mut results = Vec::new();
    for (kb_id, kb_name) in &kb_ids {
        if let Ok(search_results) = ripple_rag::store::hybrid_search(&conn, &emb, content, Some(kb_id), 3) {
            for r in search_results {
                results.push(format!("[KB: {}] {}", kb_name, r.content));
            }
        }
    }
    if results.is_empty() { None } else { Some(results.join("\n---\n")) }
}

/// 从 settings 表读取配置
async fn get_setting_str(state: &State<'_, AppState>, key: &str) -> Option<String> {
    let conn = state.db.get_timeout(Duration::from_secs(3)).ok()?;
    conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get::<_, String>(0)).ok()
}

async fn get_setting_bool(state: &State<'_, AppState>, key: &str) -> Option<bool> {
    get_setting_str(state, key).await.and_then(|v| match v.as_str() { "true" | "1" => Some(true), "false" | "0" => Some(false), _ => None })
}

async fn get_setting_int(state: &State<'_, AppState>, key: &str) -> Option<i64> {
    get_setting_str(state, key).await.and_then(|v| v.parse::<i64>().ok())
}

/// 解析系统提示中的 {键名} 占位符。
/// 从 Agents/agent_map.json 读取映射，找到对应的 .txt 文件注入内容。
fn resolve_agent_placeholders(text: &str) -> String {
    // 定位 Agents 目录
    let agents_dir = std::env::current_exe()
        .ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") { d.pop(); d.pop(); }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") { d.pop(); }
            d.join("Agents")
        }).unwrap_or_else(|| std::path::PathBuf::from("./Agents"));

    // 加载映射表
    let map_path = agents_dir.join("agent_map.json");
    let map: std::collections::HashMap<String, String> = std::fs::read_to_string(&map_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut result = text.to_string();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '}') {
                let key: String = chars[i + 1..i + 1 + end].iter().collect();
                let placeholder: String = chars[i..=i + 1 + end].iter().collect();

                // 查找映射 → filename
                let file_stem = map.get(&key).or_else(|| map.get(&key.to_lowercase()));
                if let Some(stem) = file_stem {
                    let file_path = agents_dir.join(format!("{}.txt", stem));
                    match std::fs::read_to_string(&file_path) {
                        Ok(content) => { result = result.replace(&placeholder, content.trim()); }
                        Err(_) => {
                            tracing::warn!(file = %file_path.display(), "agent txt file not found");
                            result = result.replace(&placeholder, &format!("[file missing: {stem}.txt]"));
                        }
                    }
                } else {
                    tracing::warn!(key = %key, "agent map key not found");
                    result = result.replace(&placeholder, &format!("[unknown agent: {key}]"));
                }
                i = i + 1 + end;
            }
        }
        i += 1;
    }
    result
}

struct ToolCallAccum {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}
