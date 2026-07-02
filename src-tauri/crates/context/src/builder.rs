//! ContextBuilder：上下文窗口管理核心。
//!
//! 长对话防卡顿、控成本的关键。策略：
//!   1. System Prompt 固定保留（占预算上限 10%）
//!   2. 最近 N 条消息保留原文（默认 20）
//!   3. 更早的消息按 `summary_interval` 分块生成摘要，替换原文
//!   4. 工具调用链不拆散：tool_use / tool_result 配对必须同进同出
//!   5. Token 超预算时，从最旧的摘要/消息开始裁剪
//!
//! Token 预算 = 模型上下文上限 − 预留输出。各部分占比：
//!   System 10% / Recent 70% / Summaries 15% / Reserve 5%

use std::sync::Arc;

use ripple_core::{
    ChatMessage, ContentBlock, Message, MessageRole,
};

use crate::counter::TokenCounter;
use crate::summarizer::Summarizer;

/// 预算分配比例
#[derive(Debug, Clone)]
pub struct BudgetRatio {
    pub system: f64,    // 0.10
    pub recent: f64,    // 0.70
    pub summaries: f64, // 0.15
    pub reserve: f64,   // 0.05（预留给工具结果等，不主动分配）
}

impl Default for BudgetRatio {
    fn default() -> Self {
        Self {
            system: 0.10,
            recent: 0.70,
            summaries: 0.15,
            reserve: 0.05,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextBuilderConfig {
    /// 最近保留原文的消息条数
    pub recent_window: usize,
    /// 每多少条历史消息生成一个摘要块
    pub summary_interval: usize,
    /// Token 预算分配
    pub budget: BudgetRatio,
}

impl Default for ContextBuilderConfig {
    fn default() -> Self {
        Self {
            recent_window: 20,
            summary_interval: 10,
            budget: BudgetRatio::default(),
        }
    }
}

/// 组装后的上下文
#[derive(Debug, Clone)]
pub struct AssembledContext {
    /// 最终发给模型的 messages（system prompt 已并入第一条）
    pub messages: Vec<ChatMessage>,
    pub total_tokens: usize,
    /// 是否发生了截断（前端可提示「已压缩早期对话」）
    pub truncated: bool,
    /// 用了几个摘要块
    pub summary_count: usize,
}

/// 上下文构建器
pub struct ContextBuilder {
    config: ContextBuilderConfig,
    counter: Arc<dyn TokenCounter>,
    summarizer: Arc<dyn Summarizer>,
}

impl ContextBuilder {
    pub fn new(
        config: ContextBuilderConfig,
        counter: Arc<dyn TokenCounter>,
        summarizer: Arc<dyn Summarizer>,
    ) -> Self {
        Self {
            config,
            counter,
            summarizer,
        }
    }

    /// 组装上下文。
    ///
    /// - `system_prompt`：系统提示词（固定保留）
    /// - `history`：完整历史消息（按时间正序，最早在前）
    /// - `max_context_tokens`：模型上下文上限
    /// - `reserved_output_tokens`：预留给输出的 token
    pub async fn assemble(
        &self,
        system_prompt: Option<&str>,
        history: &[Message],
        max_context_tokens: usize,
        reserved_output_tokens: usize,
    ) -> AssembledContext {
        let budget = max_context_tokens.saturating_sub(reserved_output_tokens);
        let system_budget = (budget as f64 * self.config.budget.system) as usize;
        let recent_budget = (budget as f64 * self.config.budget.recent) as usize;

        // 1. System prompt（受 system_budget 限制，超长截断）
        let system_msg = system_prompt.map(|s| {
            let truncated = truncate_text(s, system_budget, &*self.counter);
            ChatMessage {
                role: "system".into(),
                content: vec![ContentBlock::Text { text: truncated.0 }],
            }
        });

        // 2. 切分历史：尾部 recent_window 条为「最近」，其余为「待摘要」
        //    但工具调用链不能被切断 —— 从 recent_window 边界向前回溯，
        //    把「无配对的 tool_use / tool_result」纳入最近窗口。
        let split = self.find_safe_split(history);
        let to_summarize = &history[..split];
        let recent = &history[split..];

        // 3. 待摘要部分按 summary_interval 分块生成摘要
        let mut summary_blocks: Vec<String> = Vec::new();
        if !to_summarize.is_empty() {
            for chunk in to_summarize.chunks(self.config.summary_interval) {
                summary_blocks.push(self.summarizer.summarize(chunk).await);
            }
        }

        // 4. 转 ChatMessage
        let recent_msgs: Vec<ChatMessage> = recent.iter().map(message_to_chat).collect();
        let summary_msg = if summary_blocks.is_empty() {
            None
        } else {
            Some(ChatMessage {
                role: "system".into(),
                content: vec![ContentBlock::Text {
                    text: summary_blocks.join("\n---\n"),
                }],
            })
        };

        // 5. 按预算裁剪：recent 超预算时从最旧开始丢
        let (recent_msgs, recent_truncated) =
            self.fit_recent(recent_msgs, recent_budget);

        // 6. 统计
        let mut all: Vec<ChatMessage> = Vec::new();
        if let Some(s) = &system_msg {
            all.push(s.clone());
        }
        if let Some(s) = &summary_msg {
            all.push(s.clone());
        }
        all.extend(recent_msgs.iter().cloned());

        let total_tokens: usize = all
            .iter()
            .map(|m| self.counter.count_message(m))
            .sum();

        AssembledContext {
            messages: all,
            total_tokens,
            truncated: recent_truncated || system_prompt.map_or(false, |s| {
                // system 被截断也算
                self.counter.count_text(s) > system_budget
            }),
            summary_count: summary_blocks.len(),
        }
    }

    /// 找到安全的切分点：默认在 `len - recent_window`，
    /// 但若该位置切断了 tool_use/tool_result 配对，则向前回溯到配对起点。
    fn find_safe_split(&self, history: &[Message]) -> usize {
        if history.len() <= self.config.recent_window {
            return 0;
        }
        let mut split = history.len() - self.config.recent_window;

        // 向前回溯：若 history[split] 是 tool 结果/调用，且其配对在 split 之前，则回溯
        while split > 0 && breaks_tool_chain(history, split) {
            split -= 1;
        }
        split
    }

    /// recent 消息按预算裁剪：从最旧开始丢，直到总 token ≤ budget。
    /// 工具调用链同样不被拆散。
    fn fit_recent(&self, mut msgs: Vec<ChatMessage>, budget: usize) -> (Vec<ChatMessage>, bool) {
        let mut total: usize = msgs.iter().map(|m| self.counter.count_message(m)).sum();
        if total <= budget {
            return (msgs, false);
        }

        let mut truncated = false;
        // 从头部（最旧）丢弃。同样跳过会断链的消息：丢弃时若会断链，连配对一起丢。
        while total > budget && !msgs.is_empty() {
            // 找到第一个可安全丢弃的连续段
            let drop_until = self.safe_drop_end(&msgs);
            if drop_until == 0 {
                break; // 无法继续安全丢弃
            }
            let dropped: Vec<ChatMessage> = msgs.drain(..drop_until).collect();
            total = total
                .saturating_sub(dropped.iter().map(|m| self.counter.count_message(m)).sum());
            truncated = true;
        }
        (msgs, truncated)
    }

    /// 计算从头部可安全丢弃多少条（不切断 tool 链）。
    /// 返回可丢弃的条数；若第一条就不可丢，返回 0。
    fn safe_drop_end(&self, msgs: &[ChatMessage]) -> usize {
        if msgs.is_empty() {
            return 0;
        }
        // 从第 1 条开始判断：丢到第 n 条时，剩下的第一条不能是孤立的 tool_result
        // （其对应 tool_use 被丢掉了）。简化：若 msgs[0] 含 ToolResult，则它依赖更早的
        // ToolCall —— 但更早的已在待摘要区或已丢，故 ToolResult 必须与其 ToolCall 同丢。
        // 这里采用：连续丢弃直到下一条不是 ToolResult 依赖。
        let mut n = 0;
        for (i, _m) in msgs.iter().enumerate() {
            // 如果这条是 assistant 且含 ToolCall，其后的 tool 结果依赖它 → 一起丢
            n = i + 1;
            // 若下一条是 ToolResult（role=tool），说明当前 ToolCall 需带上结果，继续
            if i + 1 < msgs.len() && is_tool_result(&msgs[i + 1]) {
                continue;
            }
            // 若当前是 ToolResult，必须连同其 ToolCall（在前）一起，但 ToolCall 在前已被 n 覆盖
            break;
        }
        n
    }
}

/// history[split] 处切分会断开工具链吗？
/// 若 split 处的消息是 ToolResult，且其对应的 ToolCall 在 split 之前 → 断链。
/// 若 split 处的消息是 Assistant 含 ToolCall，且其 ToolResult 在 split 之后 → 不断（结果留在 recent）。
fn breaks_tool_chain(history: &[Message], split: usize) -> bool {
    if split >= history.len() {
        return false;
    }
    let msg = &history[split];
    // ToolResult 必须与其 ToolCall 同侧
    if msg.role == MessageRole::Tool {
        return true;
    }
    // Assistant 含 ToolCall → 检查其 ToolResult 是否在 split 之后（若是，则保持同侧 OK）
    // 这里若 msg 含 ToolCall 且后续有对应 ToolResult，split 在此处是安全的（都进 recent）。
    // 真正断链：split 之前有未配对的 ToolCall。由 ToolResult 判断覆盖。
    false
}

fn is_tool_result(msg: &ChatMessage) -> bool {
    msg.content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

fn message_to_chat(m: &Message) -> ChatMessage {
    let role = match m.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };
    ChatMessage {
        role: role.into(),
        content: m.content.clone(),
    }
}

/// 截断文本到 token 预算内。返回 (截断后文本, 是否截断)。
fn truncate_text(text: &str, budget_tokens: usize, counter: &dyn TokenCounter) -> (String, bool) {
    if counter.count_text(text) <= budget_tokens {
        return (text.to_string(), false);
    }
    // 按字符二分逼近预算
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let sub: String = chars[..mid].iter().collect();
        if counter.count_text(&sub) <= budget_tokens {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut truncated: String = chars[..lo].iter().collect();
    truncated.push('…');
    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counter::CharApproxCounter;
    use crate::summarizer::TemplateSummarizer;

    fn builder() -> ContextBuilder {
        ContextBuilder::new(
            ContextBuilderConfig {
                recent_window: 4,
                summary_interval: 2,
                budget: BudgetRatio::default(),
            },
            Arc::new(CharApproxCounter),
            Arc::new(TemplateSummarizer::default()),
        )
    }

    #[tokio::test]
    async fn short_history_kept_intact() {
        let b = builder();
        let history = vec![
            Message::new_user("c", "hello"),
            Message::new_user("c", "world"),
        ];
        let ctx = b.assemble(None, &history, 100_000, 1000).await;
        assert_eq!(ctx.summary_count, 0);
        assert!(!ctx.truncated);
        // 2 条用户消息 + 0 摘要
        assert_eq!(ctx.messages.len(), 2);
    }

    #[tokio::test]
    async fn old_messages_become_summary() {
        let b = builder();
        // recent_window=4，所以前 2 条进摘要，后 4 条保留
        let history: Vec<Message> = (0..6)
            .map(|i| Message::new_user("c", &format!("msg {i}")))
            .collect();
        let ctx = b.assemble(None, &history, 100_000, 1000).await;
        assert_eq!(ctx.summary_count, 1); // 2 条历史 / summary_interval=2 = 1 块
        assert!(ctx.messages.iter().any(|m| m.role == "system"
            && m.content.iter().any(|b| matches!(b, ContentBlock::Text { text } if text.contains("summary")))));
    }

    #[tokio::test]
    async fn budget_truncates_recent() {
        // 极小预算，迫使 recent 被裁剪
        let b = builder();
        let history: Vec<Message> = (0..4)
            .map(|i| Message::new_user("c", &format!("message number {i} is long enough")))
            .collect();
        let ctx = b.assemble(None, &history, 50, 10).await; // budget=40, recent_budget=28
        assert!(ctx.truncated);
        // 至少保留 1 条
        assert!(!ctx.messages.is_empty());
    }

    #[tokio::test]
    async fn system_prompt_preserved_and_truncated() {
        let b = builder();
        let long_prompt = "x".repeat(1000);
        let history = vec![Message::new_user("c", "hi")];
        let ctx = b.assemble(Some(&long_prompt), &history, 200, 20).await;
        assert!(ctx.truncated);
        // system 在第一条
        assert_eq!(ctx.messages[0].role, "system");
    }

    #[tokio::test]
    async fn tool_chain_not_broken() {
        // 构造：user, assistant(toolcall), tool(result), user
        // recent_window=4 时全进 recent，但若 window=2，split 处不能切断 toolcall/toolresult
        let b = ContextBuilder::new(
            ContextBuilderConfig {
                recent_window: 2,
                summary_interval: 5,
                budget: BudgetRatio::default(),
            },
            Arc::new(CharApproxCounter),
            Arc::new(TemplateSummarizer::default()),
        );
        let mut history = vec![Message::new_user("c", "first question")];
        let mut assistant = Message::new_assistant("c");
        assistant.content.push(ContentBlock::ToolCall {
            id: "tc1".into(),
            name: "search".into(),
            arguments: serde_json::json!({"q": "test"}),
        });
        history.push(assistant);
        let mut tool_msg = Message::new("c", MessageRole::Tool, vec![]);
        tool_msg.content.push(ContentBlock::ToolResult {
            tool_call_id: "tc1".into(),
            content: "result data".into(),
        });
        history.push(tool_msg);
        history.push(Message::new_user("c", "follow up"));

        let ctx = b.assemble(None, &history, 100_000, 1000).await;
        // tool_result 不应孤立出现在 recent 而其 tool_call 被摘要化
        // 检查 recent 段：若含 ToolResult，必须也含对应 ToolCall
        let recent_has_result = ctx.messages.iter().any(|m| {
            m.content.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }))
        });
        let recent_has_call = ctx.messages.iter().any(|m| {
            m.content.iter().any(|b| matches!(b, ContentBlock::ToolCall { .. }))
        });
        // 要么都没有（全进摘要），要么都有（同侧）
        assert_eq!(recent_has_result, recent_has_call);
    }
}
