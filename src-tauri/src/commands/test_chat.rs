//! 聊天测试命令：不依赖数据库，直接测试 newapi 连通性。

use ripple_core::{ChatMessage, ChatRequest, ContentBlock};
use ripple_model_provider::{ModelProvider, OpenAiProvider};
use tauri::State;

use crate::state::AppState;

/// 测试命令：直接调用 newapi，不经过数据库
#[tauri::command]
pub async fn test_chat(state: State<'_, AppState>) -> Result<String, String> {
    let api_key = crate::commands::settings::load_api_key(&state)?;
    tracing::info!("test_chat: starting direct API call");

    let provider = OpenAiProvider::new_dynamic("newapi", "newapi", "http://192.168.0.123:3000/v1");

    let request = ChatRequest {
        model: "deepseek-v4-flash".into(),
        messages: vec![ChatMessage::user("Say hello in one word")],
        system_prompt: None,
        tools: None,
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: None,
        stop_sequences: None,
    };

    tracing::info!("test_chat: sending to newapi...");

    match provider.chat(&api_key, request).await {
        Ok(response) => {
            let text: String = response
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            tracing::info!(response_chars = text.chars().count(), "test_chat completed");
            Ok(text)
        }
        Err(e) => {
            tracing::error!(error = %e, "test_chat: api call failed");
            Err(format!("API error: {e}"))
        }
    }
}
