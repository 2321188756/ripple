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

    // 2. 保存用户消息到 DB
    let user_msg = Message::new_user(&conversation_id, &content);
    { let conn = db_conn!(state); let r = ripple_conversation_store::MessageRepo::insert(&conn, &user_msg);
        if let Err(e) = r { tracing::warn!(error = %e, "save user msg failed"); } }

    // 3. 加载历史 + 构建上下文
    let conn = db_conn!(state);
    let history = ripple_conversation_store::MessageRepo::list_by_conversation(&conn, &conversation_id, 50, None).unwrap_or_default();
    drop(conn);

    let mut chat_messages: Vec<ChatMessage> = history.iter().map(|m| ChatMessage {
        role: match m.role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::Tool => "tool" }.into(),
        content: vec![ContentBlock::Text { text: m.text() }],
    }).collect();
    // 系统提示（解析 {文件名} 占位符 + @知识库 注入）
    let mut sys_text = resolve_agent_placeholders(
        conversation.system_prompt.as_deref().unwrap_or("You are a helpful AI assistant.")
    );
    let kb_inject = inject_knowledge(&content, &state, &api_key).await;
    if let Some(inject) = kb_inject { sys_text = format!("{sys_text}\n\n--- Knowledge base ---\n{inject}\n---"); }
    let agent_mode = agent_mode.unwrap_or(false);
    let sys_text = if agent_mode {
        format!("{sys_text}\n\nYou are now in AGENT mode...\nIf a tool fails, try an alternative approach.")
    } else { sys_text };

    // 上下文裁剪
    let ctx_enabled = get_setting_bool(&state, "context_enabled").await.unwrap_or(true);
    if ctx_enabled && history.len() > 10 {
        let recent_window = get_setting_int(&state, "context_recent_window").await.unwrap_or(20) as usize;
        let summary_interval = get_setting_int(&state, "context_summary_interval").await.unwrap_or(10) as usize;
        let max_tokens = get_setting_int(&state, "context_max_tokens").await.unwrap_or(32000) as usize;

        let config = ripple_context::ContextBuilderConfig { recent_window, summary_interval, budget: ripple_context::BudgetRatio::default() };
        let counter = std::sync::Arc::new(ripple_context::CharApproxCounter);
        let summarizer = std::sync::Arc::new(ripple_context::TemplateSummarizer::default());
        let builder = ripple_context::ContextBuilder::new(config, counter, summarizer);
        let ctx = builder.assemble(Some(&sys_text), &history, max_tokens, 4096).await;
        chat_messages = ctx.messages.iter().map(|m| ChatMessage { role: m.role.clone(), content: m.content.clone() }).collect();
        chat_messages.push(ChatMessage::user(&content));
        if ctx.truncated { tracing::info!(tokens = ctx.total_tokens, summaries = ctx.summary_count, "context truncated"); }
    } else {
        chat_messages.insert(0, ChatMessage { role: "system".into(), content: vec![ContentBlock::Text { text: sys_text }] });
        chat_messages.push(ChatMessage::user(&content));
    }

    // 4. 构建请求
    let model_id = model.filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if conversation.model_id == "default" { "deepseek-v4-flash".into() } else { conversation.model_id.clone() }
    });
    let base_url = api_base_url.filter(|s| !s.is_empty()).unwrap_or_else(|| "http://192.168.0.123:3000/v1".into());
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
    tracing::info!(%base_url, "request built");

    // 5. 创建 assistant 消息 ID（不存 DB，流结束后再存完整消息）
    let message_id = uuid::Uuid::new_v4().to_string();

    let active_streams = state.active_streams.clone();
    let db = state.db.clone();
    let _interrupted = state.interrupted.clone();

    // 6. 标记活跃流
    active_streams.lock().await.insert(conversation_id.clone(), crate::state::ActiveStream { conversation_id: conversation_id.clone() });

    let spawn_msg_id = message_id.clone();
    let conv_id = conversation_id.clone();
    let api_key_clone = api_key.clone();

    tracing::info!(msg_id = %spawn_msg_id, "send_message returning, spawning stream");

    // 7. 后台 tokio task
    tokio::spawn(async move {
        tracing::info!("spawned stream task");
        let result = chat_with_tools(&app, &provider, &api_key_clone, chat_request, &conv_id, &spawn_msg_id, &model_id, &db).await;

        active_streams.lock().await.remove(&conv_id);

        // 保存最终文本到 DB
        if let Ok(text) = &result {
            if !text.is_empty() {
                let msg = Message {
                    id: spawn_msg_id.clone(),
                    conversation_id: conv_id.clone(),
                    role: MessageRole::Assistant,
                    content: vec![ContentBlock::Text { text: text.clone() }],
                    created_at: chrono::Utc::now(), token_count: None,
                    metadata: serde_json::json!({}),
                };
                if let Ok(conn) = db.get_timeout(Duration::from_secs(3)) {
                    if let Err(e) = ripple_conversation_store::MessageRepo::insert(&conn, &msg) {
                        tracing::warn!(error = %e, "save assistant msg failed");
                    }
                }
            }
        }
        if let Err(e) = &result {
            tracing::error!(error = %e, "stream error");
            let _ = app.emit("chat:gen-error", GenErrorPayload { conversation_id: conv_id, message_id: spawn_msg_id, error: e.to_string() });
        }
    });

    Ok(message_id)
}

/// 停止生成
#[tauri::command]
pub async fn stop_generation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), String> {
    let mut streams = state.active_streams.lock().await;
    streams.remove(&conversation_id);
    state.interrupted.notify_waiters();
    Ok(())
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
) -> Result<String, String> {
    use tauri::Emitter;
    use std::collections::HashMap;

    let mut collected_text = String::new();
    let mut tool_calls: HashMap<u32, ToolCallAccum> = HashMap::new();

    loop {
        let mut local_text = String::new();
        tool_calls.clear();

        let stream = provider.chat_stream(api_key, request.clone()).await.map_err(|e| e.to_string())?;

        // 消费流
        consume_stream(stream, |event| {
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
                ripple_streaming::StreamEvent::Error(e) => warn!(error = %e, "stream error"),
                ripple_streaming::StreamEvent::End => {}
            }
        }).await;

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

    let conn = db.get_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let client = ripple_rag::EmbeddingClient::new(
        "http://192.168.0.123:3000/v1", api_key, "Qwen/Qwen3-Embedding-8B",
    );
    let emb = client.embed(&query).await?;
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

    let conn = state.db.get_timeout(std::time::Duration::from_secs(5)).ok()?;

    // 先查所有匹配的 KB
    let mut kb_ids: Vec<(String, String)> = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, name FROM knowledge_bases") {
        if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))) {
            for row in rows.flatten() {
                for name in &kb_names {
                    if row.1.contains(name) { kb_ids.push((row.0.clone(), row.1.clone())); break; }
                }
            }
        }
    }
    if kb_ids.is_empty() { return None; }

    // 搜索每个匹配的 KB
    let client = ripple_rag::EmbeddingClient::new("http://192.168.0.123:3000/v1", api_key, "Qwen/Qwen3-Embedding-8B");
    let emb = client.embed(content).await.ok()?;
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
