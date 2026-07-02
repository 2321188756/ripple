# ripple-context

上下文窗口管理（代码已实现，待集成）。

## 已实现

- `TokenCounter` trait + `CharApproxCounter`（chars/4 粗估）
- `Summarizer` trait + `TemplateSummarizer`（模板降级摘要）
- `ContextBuilder`：滑动窗口 + 摘要压缩 + Token 预算裁剪 + 工具链保护

## 核心算法

1. System Prompt 固定保留（10% 预算）
2. 最近 N 条保留原文（70% 预算）
3. 更早的消息分块生成摘要（15% 预算）
4. 工具调用链不拆散
5. 超预算时从最旧消息裁剪

## 测试（8 个）

- short_history_kept_intact / old_messages_become_summary / budget_truncates_recent
- system_prompt_preserved_and_truncated / tool_chain_not_broken
- approx_counter_basic / count_message_includes_overhead / template_summarizer_truncates
