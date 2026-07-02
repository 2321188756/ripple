# 性能优化策略

目标：**长对话（1000+ 消息）流式渲染依然丝滑，上下文不膨胀**。瓶颈有三处，分层治理。

## 1. 前端渲染优化

```
Raw Stream Chunks
    ↓
Chunk Buffer (Rust 端，50ms/500char flush)
    ↓
Tauri Event → Zustand Store
    ↓
React: requestAnimationFrame 批量更新（一帧一次）
    ↓
Virtual DOM: @tanstack/react-virtual（只渲染可视区）
    ↓
Markdown: 增量解析（已完成块缓存，只 parse 流式段）
```

| 策略 | 实现 | 效果 |
|------|------|------|
| 虚拟列表 | `@tanstack/react-virtual` + ResizeObserver 动态高度 | 1000 条只渲染可视区 15-20 条 DOM |
| 流式节流 | Rust 端 Buffer：50ms 或 500 字符才 emit | IPC 事件量 -90% |
| 帧对齐更新 | 收到 chunk 排队 `requestAnimationFrame`，一帧只 apply 一次 | 避免帧内多次重渲染 |
| Markdown 增量编译 | 已完成块 `useMemo` 缓存，只 parse 流式段 | 历史消息零开销 |
| 代码高亮异步 | Shiki 放 `requestIdleCallback` | 不抢 UI 线程 |
| React.memo | MessageItem 浅比较，未变旧消息跳过 | 流式时只最后一条重渲染 |
| 图片懒加载 | IntersectionObserver + 缩略图占位 | 多图不卡首屏 |

## 2. 流式传输优化

```
SSE Stream → Tokio Async Stream
    ├─ Parse SSE frame
    ├─ Extract delta / tool_call
    ├─ Accumulate in StreamBuffer
    │
    ├─ [Buffer 满 500char?] → Emit
    ├─ [距上次 emit > 50ms?] → Emit
    └─ [Stream 结束?]       → Final flush

Backpressure: 前端处理慢时 Buffer 积压 → 多 delta 合并 emit（天然反压）
```

```rust
// crates/streaming/src/bridge.rs
pub struct StreamBuffer {
    buffer: String,
    last_emit: Instant,
    min_interval: Duration,  // 50ms
    max_chars: usize,        // 500
}

impl StreamBuffer {
    pub fn push(&mut self, delta: &str) -> Option<String> {
        self.buffer.push_str(delta);
        if self.last_emit.elapsed() >= self.min_interval
            || self.buffer.len() >= self.max_chars {
            self.last_emit = Instant::now();
            Some(self.buffer.drain(..).collect())
        } else {
            None
        }
    }
    pub fn flush(&mut self) -> Option<String> { ... }
}
```

## 3. 上下文窗口管理（防卡顿核心）

长对话问题：上下文膨胀 → 每次请求 token 暴增 → API 变慢 + 费用飙升 + 内存压力。

**智能裁剪策略：**

```
完整消息历史 (SQLite)
    ├─ 1. 保留 System Prompt（固定）
    ├─ 2. 保留最近 N 条（默认 20）
    ├─ 3. 更早的 → 滑动窗口摘要
    │      每 K 条生成摘要，缓存到 messages.summary
    ├─ 4. 工具调用链保持完整（不截断 tool_use/tool_result 配对）
    └─ 5. Token 计数 → 仍超限则动态裁剪

Token 预算:
  Max Context = Model Limit - Max Output
  System Prompt: 10% | Recent: 70% | Summaries: 15% | Reserve: 5%
```

```rust
// crates/context/src/builder.rs
pub struct ContextBuilder {
    max_tokens: usize,            // 模型上下文上限
    recent_window: usize,         // 默认 20
    summary_interval: usize,      // 默认 10
    token_counter: Box<dyn TokenCounter>,
}

pub struct AssembledContext {
    pub messages: Vec<ChatMessage>,
    pub total_tokens: usize,
    pub truncated: bool,          // 前端可提示用户
    pub summary_count: usize,
}
```

**Token 计数：**
- 云端：优先用 API 返回 `usage` 缓存；新组合用 `tiktoken-rs` 估算
- 本地 Ollama：用模型返回的 token 信息

**摘要生成：**
- 首次摘要由 LLM 生成（后台异步，避免阻塞当前请求）
- 生成后缓存到 `messages.summary` 字段，复用
- 用户可在设置中关闭摘要（改为纯截断）

## 4. 数据库性能

| 优化 | 手段 |
|------|------|
| WAL 模式 | `PRAGMA journal_mode=WAL`，读写不互斥 |
| 分页加载 | 消息 cursor 分页，每次 50 条 |
| 预编译语句 | `rusqlite` 的 `prepare_cached` |
| 索引覆盖 | 常用查询走覆盖索引 |
| FTS5 | 内容卸载到 FTS 表，不重复存正文 |
| 连接池 | r2d2 复用连接，避免频繁开关 |

## 5. 内存优化

- 解密后的 API Key 用完 `zeroize`，不长期持有
- 流式响应逐块处理，不积攒完整响应
- 大文档分块处理，不全量载入内存
- 虚拟列表保证 DOM 节点数恒定
- 插件 WASM 实例按需加载，闲置卸载

## 性能验证目标

| 指标 | 目标 |
|------|------|
| 1000 条消息对话流式帧率 | > 30fps |
| 上下文裁剪后 token | ≤ 模型上限 |
| 消息列表滚动 | 无掉帧 |
| 首屏加载 | < 1s |
| RAG 检索（10 万 chunk） | < 50ms |
| 应用内存（闲置） | < 200MB |
