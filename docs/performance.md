# 性能优化策略

目标：**长对话（1000+ 消息）流式渲染依然丝滑，上下文不膨胀，首屏与设置窗口打开都快**。分层治理。

## 1. 前端渲染优化

```
Raw Stream Chunks
    ↓
Chunk Buffer (Rust 端，50ms / 500 字符 flush，按字符数非字节)
    ↓
Tauri Event → Zustand store（streamingText）
    ↓
VirtualMessageList 自订阅 streamingText → 只本组件每 token 重渲染
    ↓
Virtual DOM: @tanstack/react-virtual（只渲染可视区 + overscan 5）
    ↓
Markdown: MarkdownRenderer memo + PrismLight 按需语言
```

| 策略 | 实现 | 效果 |
|------|------|------|
| 原子 selector | App/Sidebar/ChatHeader 用 `useStore((s)=>s.field)` 精确订阅，不订阅 `streamingText` | 流式每 token 不再全树重渲染，只 VirtualMessageList 重渲染 |
| VirtualMessageList 自订阅 | 组件内 `useChatStore((s)=>s.streamingText)` + `s.messages[activeId]` | 流式文本变化隔离在消息列表，不传导 App |
| 流式节流 | Rust `StreamBuffer`：50ms 或 500 字符才 emit（`chars().count()` 非字节） | IPC 事件量 -90%，CJK 不被字节数高估 |
| 虚拟列表 | `@tanstack/react-virtual` + `measureElement` 动态高度 + overscan 5 | 1000 条只渲染可视区 15-20 条 DOM |
| React.memo | `MessageBubble` memo，接收 `content` blocks 内部 `useMemo` 提取 text/images | 旧消息零开销；不在父组件 `.filter().map()` 产生新数组击穿 memo |
| MarkdownRenderer memo | `memo()` + content 不变跳过 remark/rehype 重跑 | 历史消息零解析开销 |
| Mermaid 组件化 | `MermaidBlock` 用 state 承载 svg，动态 import mermaid | 不突变 DOM，图表按需加载 |
| 代码高亮按需 | `react-syntax-highlighter` PrismLight，只注册常用 11 种语言 | syntax-highlight chunk ~600KB → 57KB |
| 智能滚动 | `onScroll` 函数式更新 `setAutoScroll(prev => prev===next?prev:next)` | 滚动时无冗余 setState |

## 2. Bundle 分包

`vite.config.ts` 的 `manualChunks` 拆分大依赖，`build.target: "esnext"`（webview 支持最新语法，跳过降级）：

```ts
manualChunks: {
  "react-vendor": ["react", "react-dom"],
  "radix-vendor": ["@radix-ui/react-dialog", ...],
  "markdown": ["react-markdown", "remark-gfm", "remark-math", "rehype-katex"],
  "syntax-highlight": ["react-syntax-highlighter"],
}
```

效果（生产构建）：
- 主 chunk `index`：**1.5MB → 161KB**（gzip 47KB）
- `syntax-highlight`：57KB（PrismLight 只含注册语言）
- `katex` 261KB、`markdown` 434KB、`mermaid.core` 622KB、`cytoscape` 444KB 均为独立/懒加载 chunk，不进首屏主包

**设置窗口懒加载**：`main.tsx` 中 `App` 用 `lazy(() => import("./App"))`。设置窗口（`index.html#settings`）只渲染 `SettingsWindow`，不加载聊天主 bundle（markdown/katex/语法高亮/mermaid），打开速度提升 3-4 倍。设置面板组件也 `lazy` 按需加载。

## 3. 流式传输优化

```
SSE Stream → Tokio Async Stream
    ├─ Parse SSE frame (eventsource-stream)
    ├─ Extract delta / tool_call
    ├─ Accumulate in StreamBuffer (50ms / 500 字符)
    ├─ [Buffer 满 / 超 50ms / 流结束 / 控制信号] → flush + emit
    └─ Backpressure: 前端处理慢时 Buffer 积压 → 多 delta 合并 emit（天然反压）
```

```rust
// crates/streaming/src/buffer.rs
impl StreamBuffer {
    pub fn push(&mut self, delta: &str) -> Option<String> {
        self.buffer.push_str(delta);
        if self.last_emit.elapsed() >= self.config.min_interval
            || self.buffer.chars().count() >= self.config.max_chars {  // 字符数，非字节
            Some(self.drain())
        } else { None }
    }
}
```

## 4. 上下文窗口管理（防卡顿核心）

长对话问题：上下文膨胀 → 每次请求 token 暴增 → API 变慢 + 费用飙升。

**智能裁剪策略**（`crates/context/src/builder.rs`，Settings 可配置）：

```
完整消息历史 (SQLite)
    ├─ 1. 保留 System Prompt（固定）
    ├─ 2. 保留最近 N 条（默认 20）
    ├─ 3. 更早的 → 滑动窗口摘要（每 K 条生成摘要，缓存到 messages.summary）
    ├─ 4. 工具调用链保持完整（不截断 tool_use/tool_result 配对）
    └─ 5. Token 计数 → 仍超限则动态裁剪
```

**Token 计数**（`CharApproxCounter`）：`chars().count() / 4`（中英混合近似，±15%）。计数包含 Image（~85）、ToolCall、ToolResult、Thinking 块（早期版本只算 Text，预算被低估，已修）。

**摘要生成**：首次由 LLM 生成，缓存到 `messages.summary` 复用；可在设置中关闭（改纯截断）。

## 5. 数据库性能

| 优化 | 手段 |
|------|------|
| WAL 模式 | `PRAGMA journal_mode=WAL`，读写不互斥 |
| 连接池 | r2d2 max 8，`with_init` 每连接设 PRAGMA |
| foreign_keys per-connection | `with_init` 确保 CASCADE 在所有连接生效 |
| 分页加载 | 消息 `list_by_conversation(limit, offset)` |
| 预编译语句 | `prepare` + 参数绑定 |
| FTS5 | messages_fts / chunks_fts 由触发器维护 |
| 不跨 await 持连接 | embedding 等 network 调用前 drop 连接，避免池耗尽 |

## 6. 内存优化

- 流式响应逐块处理，不积攒完整响应
- 大文档分块处理，不全量载入内存
- 虚拟列表保证 DOM 节点数恒定
- 向量不常驻内存，按需从 SQLite 读

## 性能验证目标

| 指标 | 目标 |
|------|------|
| 1000 条消息对话流式帧率 | > 30fps |
| 上下文裁剪后 token | ≤ 模型上限 |
| 消息列表滚动 | 无掉帧 |
| 首屏主 chunk | < 200KB（gzip < 60KB）✅ 当前 47KB gzip |
| 设置窗口打开 | < 300ms（不含 dev 首次依赖优化）|
| 应用内存（闲置） | < 200MB |
