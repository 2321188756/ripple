# 架构设计

> 实际实现的状态文档。核心聊天链路完整可用；部分模块（WASM 插件、tool-registry 独立 crate）为未实现的规划，已在文中标注。

## 整体架构

Ripple 采用 **前后端分离 + 事件驱动** 架构：前端纯展示，所有 AI 逻辑、网络请求、文件操作在 Rust 后端完成，二者通过 Tauri IPC 通信。

```
┌──────────────────────────────────────────────────────┐
│                 Frontend (WebView)                    │
│         React 18 + TypeScript + TailwindCSS          │
│                                                       │
│  主窗口 App                  设置窗口 SettingsWindow   │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐  (独立 OS 窗口)│
│  │ Sidebar │ │ChatView  │ │Composer  │   hash 路由     │
│  │对话列表  │ │消息渲染   │ │输入发送   │   #settings    │
│  └─────────┘ └──────────┘ └──────────┘               │
│  ┌────────────────┐ ┌──────────────────────────────┐ │
│  │ MarkdownRenderer│ │ KnowledgePanel / PluginPanel │ │
│  └────────────────┘ └──────────────────────────────┘ │
│                        │                             │
│            ┌───────────┴───────────┐                 │
│            │  Tauri IPC Bridge     │  invoke/listen  │
│            └───────────┬───────────┘  emit (跨窗口)  │
└────────────────────────│─────────────────────────────┘
                         │
┌────────────────────────│─────────────────────────────┐
│                  Rust Backend                         │
│  ┌─────────────────────┴──────────────────────────┐  │
│  │              commands (IPC 处理层)               │  │
│  │ conversation│chat│message│rag_cmd│plugins│...    │  │
│  └──────┬──────────┬──────────┬──────────┬────────┘  │
│         │          │          │          │            │
│  ┌──────▼──┐ ┌─────▼────┐ ┌──▼──────┐ ┌─▼─────────┐  │
│  │ model-  │ │ context  │ │ convo-  │ │ rag       │  │
│  │ provider│ │ (裁剪)   │ │ store   │ │ (分块/检索)│  │
│  └────┬────┘ └────┬─────┘ └────┬────┘ └────┬──────┘  │
│       │          │            │            │          │
│  ┌────▼──────────▼────────────▼────────────▼──────┐  │
│  │  streaming │ security │ core(共享类型)          │  │
│  │  (节流)     │ (AES-GCM) │                       │  │
│  └────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────┘
```

## 关键设计原则

### 1. 前端纯展示
所有 AI 逻辑、API 调用、文件 I/O 在 Rust 后端。前端只负责：发送用户意图（`invoke`）、接收渲染流式更新（`listen`）、本地 UI 状态管理。

### 2. Provider 抽象
统一 `ModelProvider` trait，当前实现 `OpenAiProvider`（OpenAI 兼容，newapi 端点）。切换模型无需改对话逻辑。详见 [model-providers.md](model-providers.md)。

### 3. 流式可取消
`ActiveStream` 携带 `cancel: Arc<Notify>` + `cancelled: Arc<AtomicBool>`；`chat_with_tools` 用 `tokio::select!` 在流消费与取消信号间竞速，`stop_generation` 立即中断 HTTP 流。

### 4. API Key
当前 API Key 由前端 settingsStore 缓存、经 `send_message` 参数传后端使用。`security` crate 的 `KeyManager`（AES-256-GCM）已实现但尚未接入 `api_keys` 表（规划中）。

---

## Crate 划分

后端按职责拆分为 7 个 crate（`src-tauri/crates/`）：

| Crate | 职责 | 主要依赖 |
|-------|------|----------|
| `core` | 共享类型（Message/Conversation/ContentBlock/ToolDefinition/ProviderError）、Stream 类型 | serde, thiserror |
| `model-provider` | `ModelProvider` trait + OpenAiProvider 实现 + ProviderRegistry | reqwest, eventsource-stream |
| `streaming` | SSE 节流（StreamBuffer）+ `consume_stream` 事件桥接 | tokio-stream |
| `conversation-store` | 对话/消息 CRUD + FTS5 + 迁移 + 连接池 | rusqlite, r2d2 |
| `context` | 上下文窗口管理、滑动窗口摘要、Token 预算 | core |
| `security` | API Key 加密/解密/密钥派生（KeyManager） | aes-gcm, argon2 |
| `rag` | 文档分块、Embedding 客户端、混合检索（向量+FTS5+RRF） | core |

`commands`（在 `src-tauri/src/commands/`，非独立 crate）依赖所有业务 crate；业务 crate 依赖 `core`；`core` 不依赖任何业务 crate。

> 规划中但未实现：`tool-registry` 独立 crate（当前工具调度内联在 `chat.rs`）、WASM 插件引擎（当前插件走子进程）。

---

## 核心数据流

### 发送消息（流式 + 工具调用 + 可取消）

```
用户输入 → ChatInputArea
    │ invoke("send_message", { conversationId, content, apiKey, ... })
    ▼
commands::chat::send_message
    │ 1. conversation_store 加载对话 + 历史
    │ 2. context::builder 智能裁剪上下文（滑动窗口+摘要，可配置）
    │ 3. 收集可用工具（calculator + rag_search + 插件工具）
    │ 4. do_chat_stream_inner: spawn 后台任务
    ▼
chat_with_tools (循环，MAX_TOOL_ROUNDS=8)
    │ provider.chat_stream(api_key, request)
    │ tokio::select! {
    │   consume_stream(stream, |ev| emit("chat:stream-chunk", delta))  // StreamBuffer 节流 50ms/500char
    │   cancel.notified()  => aborted  // stop_generation 触发
    │ }
    │ [流错误] => had_error=true, break, emit("chat:gen-error"), 返回部分文本
    │ [有 tool_calls] => 执行工具, emit("chat:tool-call"), 回填结果, 继续循环
    │ [无 tool_calls] => break
    ▼
emit("chat:gen-complete", usage) + 落库助手消息
```

完整命令/事件协议见 [ipc.md](ipc.md)。

---

## 状态管理

### 后端状态（`AppState`，Tauri managed state）

```rust
pub struct AppState {
    pub db: DbPool,                                       // r2d2 SQLite 连接池 (max 8)
    pub providers: Arc<ProviderRegistry>,                 // 模型 Provider 注册表
    pub key_manager: Arc<KeyManager>,                     // API Key 加密器（暂未接入）
    pub active_streams: Arc<Mutex<HashMap<String, ActiveStream>>>, // 进行中的流（可取消）
}

pub struct ActiveStream {
    pub conversation_id: String,
    pub cancel: Arc<Notify>,        // select! 中断流
    pub cancelled: Arc<AtomicBool>, // 锁存标志，循环顶兜底
}
```

### 前端状态（Zustand stores）

| Store | 职责 |
|-------|------|
| `chatStore` | 对话列表、activeId、消息 map、toolEvents、流式状态（streamingText/streamingMsgId）、`lastActivePerAgent` |
| `agentStore` | agents 列表、selectedAgent、sidebarTab |
| `settingsStore` | apiKey / apiBaseUrl / defaultModel（持久化到 DB settings 表） |
| `kbStore` | 知识库列表、文档 map |
| `uiStore` | sidebarOpen、settingsTab（settingsOpen 在独立窗口模式下不再驱动浮层） |

**订阅约定**：用 `useStore((s) => s.field)` 原子 selector 精确订阅。`streamingText` 每 token 变化，只让 `VirtualMessageList` 订阅它；App/Sidebar/ChatHeader 不订阅，避免每 token 全树重渲染。

---

## 多窗口

设置作为独立 OS 窗口（`WebviewWindow`）：

- `openSettingsWindow()`（`src/lib/openSettings.ts`）创建 label=`settings` 的窗口，加载 `index.html#settings`
- `main.tsx` 按 `window.location.hash` 路由：`#settings` → `<SettingsWindow/>`，否则 lazy `<App/>`
- `App` 懒加载：设置窗口不加载聊天主 bundle（markdown/katex/语法高亮/mermaid），打开轻快
- 跨窗口同步：SettingsWindow 在 settingsStore/kbStore 变化时 `emit("ripple:settings-changed")`；App `listen` 后 reload settings + KB（两窗口独立 JS 上下文，store 不共享）
- 标题栏 `data-tauri-drag-region` 系统原生拖动；Escape 关闭

---

## 安全模型（Tauri v2 Capability）

`src-tauri/capabilities/default.json` 声明权限，`"windows": ["main", "settings"]` 两窗口共享：

| 权限 | 用途 |
|------|------|
| `core:default` | 基础 IPC、事件、窗口 |
| `core:webview:allow-create-webview-window` | 主窗口创建设置窗口 |
| `core:window:allow-close` / `allow-set-focus` | 设置窗口关闭/聚焦 |
| `shell:allow-open` / `dialog:default` / `dialog:allow-open` | Shell 调用、文件选择 |

前端只能 `invoke` 已声明权限的命令。
