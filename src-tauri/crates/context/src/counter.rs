//! Token 计数。trait 抽象，便于后续注入精确实现（tiktoken-rs / API usage 回填）。
//!
//! 默认 `CharApproxCounter` 用「字符数 / 4」粗估（中英混合的业界常用近似），
//! 误差约 ±15%，对上下文裁剪的预算控制足够（裁剪本就留 5% 余量）。

use ripple_core::{ChatMessage, ContentBlock};

/// Token 计数器
pub trait TokenCounter: Send + Sync {
    /// 估算一段文本的 token 数
    fn count_text(&self, text: &str) -> usize;

    /// 估算一条消息的 token 数（含角色/分隔开销）
    fn count_message(&self, msg: &ChatMessage) -> usize {
        // 每条消息约 4 token 的角色/分隔开销（OpenAI 经验值）
        let mut n = 4;
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => n += self.count_text(text),
                // 图片按 OpenAI vision 经验值估算（低细节 ~85，高细节 ~170），取 85
                ContentBlock::Image { .. } => n += 85,
                ContentBlock::ToolCall {
                    name, arguments, ..
                } => {
                    n += self.count_text(name);
                    n += self.count_text(&arguments.to_string());
                }
                ContentBlock::ToolResult { content, .. } => n += self.count_text(content),
                ContentBlock::Thinking { text } => n += self.count_text(text),
            }
        }
        n
    }
}

/// 字符近似计数器：text.len() / 4。对 ASCII 偏高、对 CJK 偏低，综合可用。
#[derive(Debug, Clone, Default)]
pub struct CharApproxCounter;

impl TokenCounter for CharApproxCounter {
    fn count_text(&self, text: &str) -> usize {
        // 用 chars().count() 而非 len()，使 CJK 字符算 1 而非 3 字节
        let chars = text.chars().count();
        chars.div_ceil(4) // 向上取整
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_counter_basic() {
        let c = CharApproxCounter;
        // 4 字符 → 1 token
        assert_eq!(c.count_text("abcd"), 1);
        // 5 字符 → 2 token（向上取整）
        assert_eq!(c.count_text("abcde"), 2);
        // 1 字符 → 1 token
        assert_eq!(c.count_text("a"), 1);
    }

    #[test]
    fn count_message_includes_overhead() {
        let c = CharApproxCounter;
        let msg = ChatMessage::user("abcdefgh"); // 8 chars → 2 token + 4 开销 = 6
        assert_eq!(c.count_message(&msg), 6);
    }
}
