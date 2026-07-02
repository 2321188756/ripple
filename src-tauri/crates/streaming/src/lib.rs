//! ripple-streaming: 流式节流与事件桥接。
//!
//! 接收 model-provider 产出的 `Stream<StreamChunk>`，用 `StreamBuffer` 合并高频
//! 文本增量，并通过回调 emit 节流后的事件。控制信号（工具调用/完成/用量）不被节流，
//! 立即透传。

pub mod buffer;

pub use buffer::{extract_signal, StreamBuffer, StreamBufferConfig, DEFAULT_MAX_CHARS, DEFAULT_MIN_INTERVAL};

use futures::StreamExt;
use ripple_core::{ProviderResult, StreamChunk};

/// 节流后的输出事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 合并后的文本增量
    Text(String),
    /// 控制信号原样透传（工具调用/思考链/完成/用量）
    Signal(StreamChunk),
    /// 上游错误
    Error(String),
    /// 流结束
    End,
}

/// 消费一个 chunk 流，节流后通过 `on_event` 回调输出。
///
/// 这是连接 Provider 流式输出与 Tauri 事件 emit 的核心粘合层。
/// 在 Tauri commands 层，`on_event` 闭包内调用 `app_handle.emit(...)`。
pub async fn consume_stream<S, F>(mut stream: S, mut on_event: F)
where
    S: futures::Stream<Item = ProviderResult<StreamChunk>> + Unpin,
    F: FnMut(StreamEvent),
{
    let mut buf = StreamBuffer::with_defaults();

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if extract_signal(&chunk) {
                    // 控制信号：先 flush 积压文本，再透传信号
                    if let Some(text) = buf.flush() {
                        on_event(StreamEvent::Text(text));
                    }
                    on_event(StreamEvent::Signal(chunk));
                } else if let Some(delta) = &chunk.delta_text {
                    if let Some(text) = buf.push(delta) {
                        on_event(StreamEvent::Text(text));
                    }
                }
            }
            Err(e) => {
                if let Some(text) = buf.flush() {
                    on_event(StreamEvent::Text(text));
                }
                on_event(StreamEvent::Error(e.to_string()));
                return;
            }
        }
    }

    // 流正常结束：flush 残留文本
    if let Some(text) = buf.flush() {
        on_event(StreamEvent::Text(text));
    }
    on_event(StreamEvent::End);
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::sync::{Arc, Mutex};

    fn text_chunk(s: &str) -> StreamChunk {
        StreamChunk {
            delta_text: Some(s.into()),
            ..Default::default()
        }
    }

    fn finish_chunk() -> StreamChunk {
        StreamChunk {
            finish_reason: Some("stop".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn consume_merges_text_and_passes_signals() {
        // 5 个文本 delta + 1 个 finish
        let chunks: Vec<ProviderResult<StreamChunk>> = vec![
            Ok(text_chunk("Hel")),
            Ok(text_chunk("lo ")),
            Ok(text_chunk("Wor")),
            Ok(text_chunk("ld")),
            Ok(finish_chunk()),
        ];
        let stream = stream::iter(chunks);

        let events = Arc::new(Mutex::new(Vec::<StreamEvent>::new()));
        let events_clone = events.clone();
        consume_stream(stream, move |e| {
            events_clone.lock().unwrap().push(e);
        })
        .await;

        let events = events.lock().unwrap();
        // 文本被合并为一条或多条（节流可能合并），最后有 Signal(finish) 和 End
        let combined_text: String = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(combined_text, "Hello World");

        assert!(matches!(events.last().unwrap(), StreamEvent::End));
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::Signal(c) if c.finish_reason.is_some())));
    }

    #[tokio::test]
    async fn consume_propagates_error_after_flush() {
        let chunks: Vec<ProviderResult<StreamChunk>> = vec![
            Ok(text_chunk("partial")),
            Err(ripple_core::ProviderError::Network("boom".into())),
        ];
        let stream = stream::iter(chunks);

        let events = Arc::new(Mutex::new(Vec::<StreamEvent>::new()));
        let events_clone = events.clone();
        consume_stream(stream, move |e| {
            events_clone.lock().unwrap().push(e);
        })
        .await;

        let events = events.lock().unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::Text(t) if t == "partial")));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::Error(_))));
        // 错误后不应有 End
        assert!(!events.iter().any(|e| matches!(e, StreamEvent::End)));
    }
}
