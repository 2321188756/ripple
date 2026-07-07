//! 聊天相关命令：发送消息、停止生成。

use std::time::Duration;

use ripple_core::{ChatMessage, ChatRequest, ContentBlock, Message, MessageRole, UsageInfo};
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
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
    user_message_id: String,
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
    // 沿用前端生成的 user 消息 id，保证前后端一致（否则前端缓存的 id 在 DB 不存在，
    // 删除/重生成等按 id 操作会 NotFound）
    let mut user_msg = Message::new(&conversation_id, MessageRole::User, blocks);
    user_msg.id = user_message_id;
    { let conn = db_conn!(state); let r = ripple_conversation_store::MessageRepo::insert(&conn, &user_msg);
        if let Err(e) = r { tracing::warn!(error = %e, "save user msg failed"); } }

    // 3. 加载历史
    let conn = db_conn!(state);
    let history = ripple_conversation_store::MessageRepo::list_by_conversation(&conn, &conversation_id, 50, None).unwrap_or_default();
    drop(conn);

    // 4. 知识库注入 + 设置
    let kb_inject = inject_knowledge(&content, &state, &api_key).await;
    tracing::debug!(kb_injected = kb_inject.is_some(), "after inject_knowledge");
    let ctx_enabled = get_setting_bool(&state, "context_enabled").await.unwrap_or(true);
    let ctx_window = get_setting_int(&state, "context_recent_window").await.unwrap_or(20) as usize;
    let ctx_interval = get_setting_int(&state, "context_summary_interval").await.unwrap_or(10) as usize;
    let ctx_max_tokens = get_setting_int(&state, "context_max_tokens").await.unwrap_or(32000) as usize;
    let default_model = get_setting_str(&state, "default_model").await
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "deepseek-v4-flash".into());

    tracing::debug!("calling do_chat_stream_inner");
    // 5. 启动流（共享逻辑）
    let (message_id, _) = do_chat_stream_inner(
        app.clone(),
        state.active_streams.clone(),
        state.db.clone(),
        state.http_client.clone(),
        &conversation, &history, &content,
        &api_key, &api_base_url, &model, &default_model, agent_mode,
        temperature, max_tokens, top_p, kb_inject,
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
    http_client: reqwest::Client,
    conversation: &ripple_core::Conversation,
    history: &[ripple_core::Message],
    content: &str,
    api_key: &str,
    api_base_url: &Option<String>,
    model: &Option<String>,
    default_model: &str,
    agent_mode: Option<bool>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
    kb_inject_text: Option<String>,
    setting_ctx_enabled: bool,
    setting_ctx_window: usize,
    setting_ctx_interval: usize,
    setting_ctx_max_tokens: usize,
) -> Result<(String, String), String> {
    use ripple_core::{ChatMessage, ChatRequest, ContentBlock, Message, MessageRole};
    use ripple_model_provider::OpenAiProvider;
    tracing::debug!("dcsi: enter");

    let model_id = model.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if conversation.model_id == "default" { default_model.to_string() } else { conversation.model_id.clone() }
    });
    // 模型是否支持图片（用模型名判断，不支持则去掉图片块）
    let supports_vision = model_id.contains("gemini") || model_id.contains("gpt-4o") || model_id.contains("vision") || model_id.contains("claude-3.5") || model_id.contains("claude-3") || model_id.contains("glm-4v");
    tracing::debug!("dcsi: before prompt_file_map");

    let mut chat_messages: Vec<ChatMessage> = history.iter().map(|m| {
        let role = match m.role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::Tool => "tool" }.into();
        let content: Vec<ContentBlock> = m.content.iter().filter_map(|block| match block {
            ContentBlock::Text { .. } => Some(block.clone()),
            ContentBlock::Image { .. } => Some(block.clone()),
            ContentBlock::ToolCall { .. } => None,
            ContentBlock::ToolResult { .. } => None,
            ContentBlock::Thinking { .. } => None,
        }).collect();
        ChatMessage { role, content: if content.is_empty() { vec![ContentBlock::Text { text: m.text() }] } else { content } }
    }).collect();

    let agent_mode = agent_mode.unwrap_or(false);
    // 读取 settings 中的 {KEY}→文件路径映射；首次使用时自动创建默认 TOOLS 映射
    let prompt_file_map: std::collections::HashMap<String, String> = (|| {
        let conn = match db.get_timeout(std::time::Duration::from_secs(3)) { Ok(c) => c, Err(_) => return std::collections::HashMap::new() };
        let existing: Option<String> = conn.query_row(
            "SELECT value FROM settings WHERE key='prompt_file_map'", [], |r| r.get(0),
        ).ok();
        let map: std::collections::HashMap<String, String> = existing
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if map.contains_key("TOOL_RULES") { return map; }
        let rules_path = project_root().join("prompts").join("tool_rules.txt").to_string_lossy().to_string();
        let mut new_map = map;
        new_map.insert("TOOL_RULES".into(), rules_path);
        if let Ok(json) = serde_json::to_string(&new_map) {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('prompt_file_map', ?1, ?2)",
                rusqlite::params![json, chrono::Utc::now().to_rfc3339()],
            );
        }
        new_map
    })();
    tracing::debug!("dcsi: after prompt_file_map");
    let mut sys_text = resolve_agent_placeholders(
        conversation.system_prompt.as_deref().unwrap_or("You are a helpful AI assistant."),
        &prompt_file_map,
    );
    tracing::debug!("dcsi: after resolve_agent_placeholders");
    if let Some(inject) = kb_inject_text { sys_text = format!("{sys_text}\n\n--- Knowledge base ---\n{inject}\n---"); }
    if agent_mode {
        sys_text = format!("{sys_text}\n\n{{TOOL_RULES}}");
        sys_text = resolve_agent_placeholders(&sys_text, &prompt_file_map);
    }

    // 记忆注入：语义检索 top-5 + 最近 10 条（去重），追加到 system prompt。
    // 按 Agent 隔离（metadata.agent_id），跨对话持久化。
    let mem_agent_id = conversation
        .metadata
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let mem_base_url = api_base_url.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    if let Some(mem_block) = crate::commands::memory::build_memory_prompt(
        &db, mem_agent_id, content, api_key, &mem_base_url,
    ).await {
        sys_text = format!("{sys_text}\n\n--- Agent Memories ---\n{mem_block}");
        tracing::debug!("memory injected");
    }

    tracing::debug!("dcsi: before ctx/manual building");

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
    tracing::debug!("dcsi: after ctx/manual building");

    // 统一过滤图片块：非视觉模型不支持 image_url content，会导致 400。
    // 早期版本只在手动构建路径过滤，context builder 路径漏过滤 → 历史含图片时必 400。
    // 这里在两条路径汇合后统一处理：非视觉模型把 Image 替换为文本占位。
    if !supports_vision {
        for msg in &mut chat_messages {
            msg.content = msg.content.iter().map(|b| match b {
                ContentBlock::Image { .. } => ContentBlock::Text { text: "[图片]".into() },
                other => other.clone(),
            }).collect();
            if msg.content.is_empty() {
                msg.content = vec![ContentBlock::Text { text: String::new() }];
            }
        }
    }

    // model_id 已在第 116 行定义
    tracing::debug!("dcsi: before provider");
    let base_url = api_base_url.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
    let provider = OpenAiProvider::with_client("newapi", "newapi", &base_url, http_client);
    tracing::debug!("dcsi: before builtin_tools");

    let chat_request = ChatRequest {
        model: model_id.clone(),
        messages: chat_messages,
        system_prompt: None,
        tools: Some(crate::commands::tools::builtin_tools()),
        temperature,
        max_tokens,
        top_p,
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
    let agent_id_clone = conversation
        .metadata
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let api_key_clone = api_key.to_string();
    let api_base_url_clone = api_base_url.clone();
    let db2 = db.clone();
    let streams2 = active_streams.clone();
    let cancel2 = cancel.clone();
    let cancelled2 = cancelled.clone();

    tokio::spawn(async move {
        tracing::info!("spawned stream task (do_chat_stream_inner)");
        let result = chat_with_tools(&app, &provider, &api_key_clone, &api_base_url_clone, chat_request, &conv_id2, &agent_id_clone, &spawn_msg_id, &model_id, &db2, &cancel2, &cancelled2).await;

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
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
) -> Result<String, String> {
    tracing::info!(%conversation_id, %message_id, "regenerate start");

    // 0. 停止该会话进行中的流。否则旧流的 spawned task 仍在写 DB（保存助手消息），
    //    与本次 delete_from 的写锁竞争，busy_timeout 内拿不到锁 → "db timeout"。
    {
        let stream = state.active_streams.lock().await.remove(&conversation_id);
        if let Some(s) = stream {
            s.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            s.cancel.notify_waiters();
            tracing::debug!("regenerate: stopped in-flight stream");
        }
    }
    tracing::debug!("regenerate: after stop_stream");

    // 1. 获取对话
    let conn = db_conn!(state);
    let conversation = match conversation::get_conversation_inner(&conn, &conversation_id) {
        Ok(c) => c,
        Err(e) => return Err(format!("conversation not found: {e}")),
    };
    drop(conn);
    tracing::debug!("regenerate: after get_conversation");

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
    tracing::debug!("regenerate: after delete_from");

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
    tracing::debug!(kb_injected = kb_inject.is_some(), "regenerate after inject_knowledge");
    let ctx_enabled = get_setting_bool(&state, "context_enabled").await.unwrap_or(true);
    let ctx_window = get_setting_int(&state, "context_recent_window").await.unwrap_or(20) as usize;
    let ctx_interval = get_setting_int(&state, "context_summary_interval").await.unwrap_or(10) as usize;
    let ctx_max_tokens = get_setting_int(&state, "context_max_tokens").await.unwrap_or(32000) as usize;
    let default_model = get_setting_str(&state, "default_model").await
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "deepseek-v4-flash".into());

    tracing::debug!("regenerate calling do_chat_stream_inner");
    // 6. 复用核心流逻辑
    let (msg_id, _) = do_chat_stream_inner(
        app.clone(),
        state.active_streams.clone(),
        state.db.clone(),
        state.http_client.clone(),
        &conversation, &history, &content,
        &api_key, &api_base_url, &model, &default_model, agent_mode,
        temperature, max_tokens, top_p, kb_inject,
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
/// 检查工具是否需要用户审批，返回 (needs_approval, permission_level)。
/// needs_approval=false 表示 Agent 权限级别（full）或已信任该工具，可自动放行。
async fn check_tool_approval(
    db: &ripple_conversation_store::DbPool,
    agent_id: &str,
    tool_name: &str,
) -> (bool, String) {
    use crate::commands::plugins::{tool_requires_approval, agent_permission_level, is_tool_trusted};
    if !tool_requires_approval(tool_name) {
        return (false, String::new());
    }
    let level = match db.get_timeout(Duration::from_secs(3)) {
        Ok(conn) => agent_permission_level(&conn, agent_id),
        Err(_) => "strict".into(),
    };
    let need = match level.as_str() {
        "full" => false,
        "elevated" => {
            let trusted = match db.get_timeout(Duration::from_secs(3)) {
                Ok(conn) => is_tool_trusted(&conn, agent_id, tool_name),
                Err(_) => false,
            };
            !trusted
        }
        _ => true, // strict
    };
    tracing::info!(agent_id, tool_name, %level, need, "approval decision");
    (need, level)
}

/// 请求用户审批工具调用。emit `chat:tool-approval-request` 事件，阻塞等待前端 approve_tool_call 回传（120s 超时）。
/// 返回 (approved, trust_tool)：trust_tool=true 表示用户勾选了「信任此工具」（elevated 模式下记录到信任表）。
async fn request_tool_approval(
    app: &AppHandle,
    conversation_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
    permission_level: &str,
) -> Result<(bool, bool), String> {
    use tauri::Emitter;
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    crate::commands::plugins::register_pending_approval(&request_id, tx);
    let payload = serde_json::json!({
        "request_id": request_id,
        "conversation_id": conversation_id,
        "tool_name": tool_name,
        "arguments": args,
        "permission_level": permission_level,
    });
    app.emit("chat:tool-approval-request", payload)
        .map_err(|e| format!("emit approval request: {e}"))?;
    tracing::info!(%request_id, tool_name, "approval request emitted, waiting for user");
    match tokio::time::timeout(Duration::from_secs(120), rx).await {
        Ok(Ok(v)) => {
            tracing::info!(%request_id, approved = v.0, trust_tool = v.1, "approval resolved");
            Ok(v)
        }
        Ok(Err(_)) => Err("approval channel closed".into()),
        Err(_) => {
            // 超时：清理 pending，避免 Sender 泄漏
            crate::commands::plugins::take_pending_approval(&request_id);
            Err("approval timeout (120s 无响应)".into())
        }
    }
}

async fn chat_with_tools(
    app: &AppHandle,
    provider: &OpenAiProvider,
    api_key: &str,
    api_base_url: &Option<String>,
    mut request: ChatRequest,
    conversation_id: &str,
    agent_id: &str,
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
                        tracing::debug!(len = text.len(), "stream text delta");
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
                                tracing::debug!(idx = tc.index, id = ?tc.id, name = ?tc.name, "tool_call delta");
                                let e = tool_calls.entry(tc.index).or_insert_with(|| ToolCallAccum { id: None, name: None, arguments: String::new() });
                                if let Some(id) = tc.id { e.id = Some(id); }
                                if let Some(name) = tc.name { e.name = Some(name); }
                                if let Some(frag) = tc.arguments_fragment { e.arguments.push_str(&frag); }
                            }
                        }
                        if let Some(reason) = chunk.finish_reason {
                            tracing::debug!(reason = %reason, "stream finish");
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

            // 审批门控：按 Agent 权限级别（strict/elevated/full）+ 信任表决定是否需用户确认
            if crate::commands::plugins::tool_requires_approval(name) {
                let (need, level) = check_tool_approval(db, agent_id, name).await;
                if need {
                    match request_tool_approval(app, conversation_id, name, &args, &level).await {
                        Ok((true, trust_tool)) => {
                            // elevated 模式下用户勾选「信任此工具」→ 记录，后续该工具自动放行
                            if trust_tool && level == "elevated" {
                                if let Ok(conn) = db.get_timeout(Duration::from_secs(3)) {
                                    let _ = crate::commands::plugins::add_trusted_tool(&conn, agent_id, name);
                                }
                            }
                        }
                        Ok((false, _)) => {
                            let output = "用户拒绝了此次工具调用".to_string();
                            let tid = tc.id.clone().unwrap_or_default();
                            request.messages.push(ChatMessage {
                                role: "tool".into(),
                                content: vec![ContentBlock::ToolResult { tool_call_id: tid, content: output.clone() }],
                            });
                            let _ = app.emit("chat:tool-call", serde_json::json!({
                                "tool_name": name, "tool_input": tc.arguments, "tool_output": output, "status": "rejected"
                            }));
                            tracing::info!(tool = %name, "tool call rejected by user");
                            continue;
                        }
                        Err(e) => {
                            let output = format!("审批失败: {e}");
                            let tid = tc.id.clone().unwrap_or_default();
                            request.messages.push(ChatMessage {
                                role: "tool".into(),
                                content: vec![ContentBlock::ToolResult { tool_call_id: tid, content: output.clone() }],
                            });
                            let _ = app.emit("chat:tool-call", serde_json::json!({
                                "tool_name": name, "tool_input": tc.arguments, "tool_output": output, "status": "error"
                            }));
                            tracing::warn!(tool = %name, error = %e, "tool approval failed");
                            continue;
                        }
                    }
                }
                // need=false：full 模式或已信任，直接执行（落到下面 exec）
            }

            let exec = match name {
                "calculator" => crate::commands::tools::exec_calculator(&args),
                "rag_search" => exec_rag_search(db, api_key, &args).await,
                "get_time_info" => Ok(crate::commands::tools::exec_get_time_info(&args)?),
                "get_weather" => crate::commands::tools::exec_get_weather(&args).await,
                "remember" => crate::commands::memory::exec_remember(db, conversation_id, &args).await,
                other if other.starts_with("plugin_") => {
                    crate::commands::plugins::exec_by_tool_name(other, &args, Some(api_key), api_base_url.as_deref()).await
                }
                other => Err(format!("unknown tool: {other}")),
            };
            let (status, output): (String, String) = match &exec {
                Ok(v) => ("success".to_string(), v.clone()),
                Err(e) => ("error".to_string(), e.clone()),
            };

            tracing::info!(tool = %name, status = %status, "tool executed");
            // debug 模式下记录工具入参/出参（可能很长，仅 debug）
            tracing::debug!(tool = %name, input = %tc.arguments, output = %output, "tool call detail");

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
    )?;
    // 先获取嵌入（网络调用，可能耗时数秒），不持有 DB 连接，避免连接池耗尽。
    // 加 20s 超时，避免工具调用因 embed 挂起而长时间无响应。
    let emb = tokio::time::timeout(Duration::from_secs(20), client.embed(&query))
        .await
        .map_err(|_| "embedding timeout (20s)".to_string())??;
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

    // 嵌入查询（网络调用），不持有 DB 连接。
    // 加 15s 超时：embedding 端点慢/挂起时不应阻塞 send_message 返回（会触发前端 30s IPC 超时）。
    // 超时则跳过注入（降级，本次回答不带知识库上下文）。
    let client = ripple_rag::EmbeddingClient::new("http://192.168.0.123:3000/v1", api_key, "Qwen/Qwen3-Embedding-8B").ok()?;
    let emb = match tokio::time::timeout(Duration::from_secs(15), client.embed(content)).await {
        Ok(Ok(e)) => e,
        Ok(Err(e)) => { tracing::warn!(error = %e, "inject_knowledge embed failed, skipping"); return None; }
        Err(_) => { tracing::warn!("inject_knowledge embed timeout 15s, skipping"); return None; }
    };

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
/// 获取项目根目录（从 current_exe 上溯）
fn project_root() -> std::path::PathBuf {
    std::env::current_exe()
        .ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") { d.pop(); d.pop(); }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") { d.pop(); }
            d
        }).unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn resolve_agent_placeholders(
    text: &str,
    file_overrides: &std::collections::HashMap<String, String>,
) -> String {
    // 定位 Agents 目录
    let agents_dir = std::env::current_exe()
        .ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|mut d| {
            if d.ends_with("debug") || d.ends_with("release") { d.pop(); d.pop(); }
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") { d.pop(); }
            d.join("Agents")
        }).unwrap_or_else(|| std::path::PathBuf::from("./Agents"));

    // 加载 Agents/agent_map.json 映射
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

                // 优先查 settings 中的文件映射（prompt_file_map）
                if let Some(file_path) = file_overrides.get(&key) {
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        result = result.replace(&placeholder, content.trim());
                        // 不能 i=0 重启扫描：chars 是原始文本，重启会再次定位到同一占位符，
                        // result.replace 已无该占位符（no-op），但 i=0; continue 死循环 + 反复读文件。
                        // result.replace 已替换所有出现，直接落到下方 i = i+1+end 前进即可。
                    }
                }
                // 再查 Agents/agent_map.json 映射
                if let Some(stem) = map.get(&key).or_else(|| map.get(&key.to_lowercase())) {
                    let file_path = agents_dir.join(format!("{stem}.txt"));
                    match std::fs::read_to_string(&file_path) {
                        Ok(content) => { result = result.replace(&placeholder, content.trim()); }
                        Err(_) => {
                            tracing::warn!(file = %file_path.display(), "agent txt file not found");
                            result = result.replace(&placeholder, &format!("[file missing: {stem}.txt]"));
                        }
                    }
                } else {
                    // settings 映射也没找到且 agent_map.json 也没有 → 保留原样（可能是普通文本）
                    if !file_overrides.contains_key(&key) {
                        tracing::warn!(key = %key, "placeholder not resolved");
                    }
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
