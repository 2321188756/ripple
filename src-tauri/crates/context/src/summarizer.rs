//! 摘要生成。trait 抽象，真实场景由 LLM 生成；无 LLM 时用 `TemplateSummarizer` 降级。
//!
//! 摘要缓存到 `messages.summary` 字段，避免重复生成。

use async_trait::async_trait;
use ripple_core::Message;

/// 摘要生成器
#[async_trait]
pub trait Summarizer: Send + Sync {
    /// 将一批消息压缩为一段摘要文本
    async fn summarize(&self, messages: &[Message]) -> String;
}

/// 模板降级摘要：拼接每条消息的角色 + 前 N 字符，超长截断。
/// 不依赖 LLM，保证离线/降级时上下文裁剪仍可用。
pub struct TemplateSummarizer {
    /// 每条消息最多取多少字符进入摘要
    pub per_message_chars: usize,
}

impl Default for TemplateSummarizer {
    fn default() -> Self {
        Self {
            per_message_chars: 200,
        }
    }
}

#[async_trait]
impl Summarizer for TemplateSummarizer {
    async fn summarize(&self, messages: &[Message]) -> String {
        let mut out = String::from("[Earlier conversation summary]\n");
        for m in messages {
            let role = match m.role {
                ripple_core::MessageRole::System => "system",
                ripple_core::MessageRole::User => "user",
                ripple_core::MessageRole::Assistant => "assistant",
                ripple_core::MessageRole::Tool => "tool",
            };
            let text = m.text();
            let truncated: String = text.chars().take(self.per_message_chars).collect();
            out.push_str(&format!("{role}: {truncated}"));
            if text.chars().count() > self.per_message_chars {
                out.push('…');
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn template_summarizer_truncates() {
        let s = TemplateSummarizer {
            per_message_chars: 5,
        };
        let m1 = Message::new_user("c1", "hello world this is long");
        let summary = s.summarize(&[m1]).await;
        assert!(summary.contains("user: hello…"));
    }
}
