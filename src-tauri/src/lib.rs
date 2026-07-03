//! Ripple Tauri 应用入口。

use std::path::PathBuf;
use tauri::Manager;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod state;

pub use state::AppState;

pub fn run() {
    // 日志目录：项目根下的 logs/
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|p| {
            // 在开发模式下 (target/debug/)，日志放到项目根
            let mut d = p;
            // 上两级：target/debug/ → project root
            if d.ends_with("debug") || d.ends_with("release") {
                d.pop(); d.pop();
            }
            // 再往上一级：src-tauri/ → project root
            if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
                d.pop();
            }
            d.join("logs")
        })
        .unwrap_or_else(|| std::path::PathBuf::from("./logs"));
    std::fs::create_dir_all(&log_dir).ok();
    commands::log::set_log_dir(log_dir.clone());

    // 日志：控台 + 文件双输出
    let file_appender = tracing_appender::rolling::daily(&log_dir, "ripple.log");
    let (file_writer, _file_guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::new("ripple_app=debug,ripple=debug,ripple_core=debug,ripple_model_provider=debug,ripple_streaming=debug,ripple_security=debug,ripple_context=debug,ripple_conversation_store=debug,warn"))
        .with(fmt::Layer::new().with_writer(std::io::stdout).with_target(true))
        .with(
            fmt::Layer::new()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(true),
        )
        .init();

    tracing::info!("=== Ripple starting ===");
    tracing::info!(log_dir = %log_dir.display());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // 数据目录放项目根（同日志目录）
            let data_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .map(|p| {
                    let mut d = p;
                    if d.ends_with("debug") || d.ends_with("release") { d.pop(); d.pop(); }
                    if d.file_name().and_then(|s| s.to_str()) == Some("src-tauri") { d.pop(); }
                    d
                })
                .unwrap_or_else(|| PathBuf::from("."));
            std::fs::create_dir_all(&data_dir).ok();
            tracing::info!(data_dir = %data_dir.display());

            // 数据库
            let db_path = data_dir.join("ripple.db");
            let db_pool = ripple_conversation_store::init_db(&db_path)
                .expect("failed to initialize database");
            tracing::info!("database ready");

            // 注册表
            let providers = ripple_model_provider::ProviderRegistry::with_builtins();
            let key_manager = ripple_security::KeyManager::new("ripple-dev-machine", None)
                .expect("failed to init key manager");
            tracing::info!("provider registry + key manager ready");

            let state = AppState {
                db: db_pool,
                providers: std::sync::Arc::new(providers),
                key_manager: std::sync::Arc::new(key_manager),
                active_streams: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
            };

            app.manage(state);
            tracing::info!("=== Ripple setup complete ===");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agents::list_agents,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::delete_agent,
            commands::agents::get_agent,
            commands::rag_cmd::create_kb,
            commands::rag_cmd::list_kbs,
            commands::rag_cmd::delete_kb,
            commands::rag_cmd::list_docs,
            commands::rag_cmd::import_document,
            commands::rag_cmd::search_kb,
            commands::rag_cmd::delete_document,
            commands::rag_cmd::get_document_content,
            commands::rag_cmd::update_document_content,
            commands::rag_cmd::import_folder,
            commands::rag_cmd::batch_delete_documents,
            commands::rag_cmd::rename_document,
            commands::test::ping,
            commands::test_chat::test_chat,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::stats::get_usage_stats,
            commands::plugins::list_plugins,
            commands::plugins::toggle_plugin,
            commands::plugins::execute_plugin_tool,
            commands::plugins::get_plugin_config,
            commands::plugins::set_plugin_config,
            commands::log::log_event,
            commands::log::get_log_path,
            commands::log::get_logs,
            commands::chat::send_message,
            commands::chat::stop_generation,
            commands::chat::regenerate,
            commands::export::export_conversation,
            commands::export::import_conversation,
            commands::conversation::create_conversation,
            commands::conversation::list_conversations,
            commands::conversation::delete_conversation,
            commands::conversation::get_conversation,
            commands::conversation::update_conversation,
            commands::message::get_messages,
            commands::message::search_messages,
            commands::message::update_message,
            commands::message::delete_messages_from,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
