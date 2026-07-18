//! 应用共享状态，Tauri managed state。

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ripple_conversation_store::DbPool;
use ripple_model_provider::ProviderRegistry;
use ripple_security::KeyManager;
use tokio::sync::{Mutex, Notify};

/// 进行中的流式生成，可用于取消。
/// `cancel` 用于 select! 中唤醒并中断流；`cancelled` 为锁存标志，
/// 即便 notify 在首次 poll 前发出（极小竞态窗口）也能在循环顶被捕获。
pub struct ActiveStream {
    pub stream_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub cancel: Arc<Notify>,
    pub cancelled: Arc<AtomicBool>,
}

pub struct AppState {
    pub db: DbPool,
    pub providers: Arc<ProviderRegistry>,
    pub key_manager: Arc<KeyManager>,
    pub active_streams: Arc<Mutex<HashMap<String, ActiveStream>>>,
    /// 共享 HTTP 客户端（连接池复用，避免每请求新建 + TLS 握手）。
    /// reqwest::Client 内部 Arc，clone 廉价。
    pub http_client: reqwest::Client,
}
