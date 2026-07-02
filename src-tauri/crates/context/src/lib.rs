//! ripple-context: 上下文窗口管理。
//!
//! 长对话防卡顿、控成本的核心。滑动窗口 + 摘要压缩 + Token 预算裁剪，
//! 并保证工具调用链不被拆散。

pub mod builder;
pub mod counter;
pub mod summarizer;

pub use builder::{AssembledContext, BudgetRatio, ContextBuilder, ContextBuilderConfig};
pub use counter::{CharApproxCounter, TokenCounter};
pub use summarizer::{Summarizer, TemplateSummarizer};
