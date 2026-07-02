//! 应用共享状态，Tauri managed state。

use std::collections::HashMap;
use std::sync::Arc;

use ripple_conversation_store::DbPool;
use ripple_model_provider::ProviderRegistry;
use ripple_security::KeyManager;
use tokio::sync::{Mutex, Notify};

/// 进行中的流式生成，可用于取消
pub struct ActiveStream {
    pub conversation_id: String,
}

pub struct AppState {
    pub db: DbPool,
    pub providers: Arc<ProviderRegistry>,
    pub key_manager: Arc<KeyManager>,
    pub active_streams: Arc<Mutex<HashMap<String, ActiveStream>>>,
    /// 通知后台流式任务停止
    pub interrupted: Arc<Notify>,
}
