//! StreamBuffer: 流式节流核心。
//!
//! 设计目标：把高频 SSE delta 合并，降低 IPC 事件数量（目标 -90%）。
//!
//! 触发 flush 的条件（任一满足）：
//!   1. 距上次 emit 超过 `min_interval`（默认 50ms）
//!   2. 累积字符数达到 `max_chars`（默认 500）
//!   3. 流结束 / 收到非文本 chunk（工具调用等需立即 flush）
//!
//! 天然反压：前端处理慢时 buffer 积压，多 delta 自动合并为一次 emit。

use std::time::{Duration, Instant};

use ripple_core::StreamChunk;

/// 默认节流参数
pub const DEFAULT_MIN_INTERVAL: Duration = Duration::from_millis(50);
pub const DEFAULT_MAX_CHARS: usize = 500;

#[derive(Debug, Clone)]
pub struct StreamBufferConfig {
    pub min_interval: Duration,
    pub max_chars: usize,
}

impl Default for StreamBufferConfig {
    fn default() -> Self {
        Self {
            min_interval: DEFAULT_MIN_INTERVAL,
            max_chars: DEFAULT_MAX_CHARS,
        }
    }
}

/// 文本增量缓冲器。
///
/// 只缓冲 `delta_text`；`tool_calls` / `finish_reason` / `usage` 等控制信号
/// 不应被节流，调用方需先 `flush()` 取出积压文本，再单独处理控制信号。
pub struct StreamBuffer {
    buffer: String,
    last_emit: Instant,
    config: StreamBufferConfig,
}

impl StreamBuffer {
    pub fn new(config: StreamBufferConfig) -> Self {
        Self {
            buffer: String::new(),
            last_emit: Instant::now(),
            config,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(StreamBufferConfig::default())
    }

    /// 推入一段文本，返回应立即 flush 的文本（满足触发条件时）。
    /// 返回 `Some(text)` 表示调用方应 emit 这段文本。
    pub fn push(&mut self, delta: &str) -> Option<String> {
        if delta.is_empty() {
            return None;
        }
        self.buffer.push_str(delta);

        if self.last_emit.elapsed() >= self.config.min_interval
            || self.buffer.len() >= self.config.max_chars
        {
            Some(self.drain())
        } else {
            None
        }
    }

    /// 强制取出积压文本（流结束或遇到控制信号时调用）。
    pub fn flush(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(self.drain())
        }
    }

    fn drain(&mut self) -> String {
        let out = std::mem::take(&mut self.buffer);
        self.last_emit = Instant::now();
        out
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// 从原始 chunk 流中提取需立即处理的控制信号。
/// 返回 `Some(chunk)` 表示该 chunk 不是普通文本增量，不应被节流。
pub fn extract_signal(chunk: &StreamChunk) -> bool {
    chunk.tool_calls.is_some()
        || chunk.finish_reason.is_some()
        || chunk.usage.is_some()
        || chunk.delta_thinking.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_holds_until_interval_or_size() {
        let mut buf = StreamBuffer::new(StreamBufferConfig {
            min_interval: Duration::from_secs(60), // 永远不因时间触发
            max_chars: 10,
        });

        // 未达阈值，不 flush
        assert_eq!(buf.push("hi"), None);
        assert_eq!(buf.push("there"), None); // 7 chars
        // 达到 10 chars
        assert_eq!(buf.push("!!!!"), Some("hithere!!!!".to_string()));
        assert!(buf.is_empty());
    }

    #[test]
    fn flush_drains_remaining() {
        let mut buf = StreamBuffer::with_defaults();
        assert_eq!(buf.push("a"), None); // 刚 emit 过，时间未到且未达 size
        let drained = buf.flush();
        assert_eq!(drained, Some("a".to_string()));
        assert_eq!(buf.flush(), None);
    }

    #[test]
    fn empty_push_returns_none() {
        let mut buf = StreamBuffer::with_defaults();
        assert_eq!(buf.push(""), None);
    }
}
