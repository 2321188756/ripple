//! Ripple Tauri 应用入口。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tracing_subscriber::{fmt, prelude::*, reload, EnvFilter, Registry};

mod commands;
mod state;

pub use state::AppState;

/// 运行时日志级别切换 handle（reload layer），存 OnceCell 供 set_debug_enabled 访问。
static LOG_RELOAD: once_cell::sync::OnceCell<reload::Handle<EnvFilter, Registry>> =
    once_cell::sync::OnceCell::new();

/// Debug 模式开关。true 时输出请求体/流式 chunk/工具调用等细节日志。
static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

const INFO_FILTER: &str = "ripple_app=info,ripple=info,ripple_core=info,ripple_model_provider=info,ripple_streaming=info,ripple_security=info,ripple_context=info,ripple_conversation_store=info,warn";
const DEBUG_FILTER: &str = "ripple_app=debug,ripple=debug,ripple_core=debug,ripple_model_provider=debug,ripple_streaming=debug,ripple_security=debug,ripple_context=debug,ripple_conversation_store=debug,trace";
const DATA_DIR_ENV: &str = "RIPPLE_DATA_DIR";

fn inferred_data_dir(executable: Option<PathBuf>) -> PathBuf {
    executable
        .and_then(|path| path.parent().map(PathBuf::from))
        .map(|mut path| {
            if path.ends_with("debug") || path.ends_with("release") {
                path.pop();
                path.pop();
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("src-tauri") {
                path.pop();
            }
            path
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn data_dir_from_override(
    override_value: Option<std::ffi::OsString>,
    executable: Option<PathBuf>,
) -> PathBuf {
    override_value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| inferred_data_dir(executable))
}

fn runtime_data_dir() -> PathBuf {
    data_dir_from_override(std::env::var_os(DATA_DIR_ENV), std::env::current_exe().ok())
}

/// 运行时切换 debug 日志。同时更新全局开关（供 if debug { ... } 场景）+ reload filter 级别。
pub fn set_debug_enabled(enabled: bool) {
    DEBUG_MODE.store(enabled, Ordering::SeqCst);
    if let Some(handle) = LOG_RELOAD.get() {
        let f = if enabled {
            EnvFilter::new(DEBUG_FILTER)
        } else {
            EnvFilter::new(INFO_FILTER)
        };
        let _ = handle.modify(|cur| *cur = f);
    }
    tracing::info!(enabled, "debug logging toggled");
}

/// 当前是否开启 debug 模式。
pub fn debug_enabled() -> bool {
    DEBUG_MODE.load(Ordering::SeqCst)
}

pub fn run() {
    let data_dir = runtime_data_dir();
    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    commands::log::set_log_dir(log_dir.clone());

    // 日志：控台 + 文件双输出。filter 用 reload::Layer 包裹，支持运行时切换 info↔debug。
    let file_appender = tracing_appender::rolling::daily(&log_dir, "ripple.log");
    let (file_writer, _file_guard) = tracing_appender::non_blocking(file_appender);

    let (filter_layer, reload_handle) = reload::Layer::new(EnvFilter::new(INFO_FILTER));
    let _ = LOG_RELOAD.set(reload_handle);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(
            fmt::Layer::new()
                .with_writer(std::io::stdout)
                .with_target(true),
        )
        .with(
            fmt::Layer::new()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(true),
        )
        .init();

    tracing::info!("=== Ripple starting ===");
    tracing::info!(log_dir = %log_dir.display());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build());
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio::init());

    builder
        .setup(move |app| {
            std::fs::create_dir_all(&data_dir).expect("failed to create Ripple data directory");
            tracing::info!(data_dir = %data_dir.display());

            // 数据库
            let db_path = data_dir.join("ripple.db");
            let db_pool = ripple_conversation_store::init_db(&db_path)
                .expect("failed to initialize database");
            tracing::info!("database ready");

            // 读取 debug_logging 设置，初始化日志级别
            {
                if let Ok(conn) = db_pool.get_timeout(std::time::Duration::from_secs(3)) {
                    let v: Option<String> = conn
                        .query_row(
                            "SELECT value FROM settings WHERE key='debug_logging'",
                            [],
                            |r| r.get(0),
                        )
                        .ok();
                    if v.as_deref() == Some("true") {
                        set_debug_enabled(true);
                    }
                }
            }

            // 每个安装使用独立且持久的随机 secret；已有文件损坏时拒绝启动，避免生成新值导致密文不可恢复。
            let install_secret_path = data_dir.join(".ripple-install-secret");
            let install_secret = match std::fs::read_to_string(&install_secret_path) {
                Ok(secret) if secret.trim().len() >= 32 => secret,
                Ok(_) => panic!("invalid Ripple install secret"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let secret = uuid::Uuid::new_v4().to_string();
                    std::fs::write(&install_secret_path, &secret)
                        .expect("failed to persist install secret");
                    secret
                }
                Err(error) => panic!("failed to read Ripple install secret: {error}"),
            };

            // 注册表
            let providers = ripple_model_provider::ProviderRegistry::with_builtins();
            let key_manager = ripple_security::KeyManager::new(install_secret.trim(), None)
                .expect("failed to init key manager");
            let key_manager = std::sync::Arc::new(key_manager);
            commands::settings::migrate_legacy_api_key(&db_pool, &key_manager)
                .unwrap_or_else(|error| panic!("failed to migrate legacy API key: {error}"));
            tracing::info!("provider registry + key manager ready");

            let state = AppState {
                db: db_pool,
                providers: std::sync::Arc::new(providers),
                key_manager,
                active_streams: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
                http_client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                    .expect("failed to build http client"),
            };

            app.manage(state);
            tracing::info!("=== Ripple setup complete ===");

            // 启动时扫描插件填充注册表。plugin_tools() 从注册表取工具注入 LLM，
            // 若不在此扫描，未打开过 Plugins 面板时注册表为空，AI 看不到插件工具。
            let loaded = commands::plugins::scan_plugins();
            tracing::info!(count = loaded.len(), plugins = ?loaded, "plugins loaded at startup");

            // 启动时后台索引所有 Agent 的记忆（fire-and-forget，不阻塞启动）
            {
                let db_clone = app.state::<AppState>().db.clone();
                let key_manager = app.state::<AppState>().key_manager.clone();
                tauri::async_runtime::spawn(async move {
                    crate::commands::memory::index_all_agents(db_clone, key_manager).await;
                });
            }
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
            commands::settings::save_api_key,
            commands::settings::has_api_key,
            commands::settings::clear_api_key,
            commands::settings::set_debug_logging,
            commands::settings::get_debug_logging,
            commands::settings::list_available_models,
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
            commands::memory::reindex_memories,
            commands::memory::list_memory_files,
            commands::memory::get_memory_file,
            commands::memory::delete_memory_file,
            commands::memory::memory_stats,
            commands::memory::open_memory_dir,
            commands::memory::memory_overview,
            commands::memory::save_memory_file,
            commands::memory::generate_memory_tags,
            commands::plugins::approve_tool_call,
            commands::plugins::get_agent_permission_level,
            commands::plugins::set_agent_permission_level,
            commands::plugins::list_trusted_tools,
            commands::plugins::revoke_trust,
            commands::themes::list_themes,
            commands::themes::save_themes,
            commands::themes::export_theme,
            commands::themes::import_theme,
            commands::themes::delete_theme,
            commands::themes::generate_theme,
            commands::themes::save_wallpaper,
            commands::themes::read_wallpaper_base64,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inferred_data_dir_preserves_existing_layout() {
        let executable = PathBuf::from("C:/workspace/Ripple/src-tauri/target/debug/ripple.exe");
        assert_eq!(
            inferred_data_dir(Some(executable)),
            PathBuf::from("C:/workspace/Ripple")
        );
    }

    #[test]
    fn explicit_data_dir_value_is_used_verbatim() {
        let value = PathBuf::from("C:/temp/ripple-e2e");
        assert_eq!(
            data_dir_from_override(
                Some(value.as_os_str().to_os_string()),
                Some(PathBuf::from("C:/ignored/ripple.exe")),
            ),
            value
        );
    }
}
