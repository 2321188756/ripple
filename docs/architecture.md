# 架构设计

> **状态：** 本文档为设计蓝图，部分模块（WASM 插件引擎、tool-registry 独立 crate）尚未实现，实际实现集中在核心聊天链路。

## 整体架构

Ripple 采用 **前后端分离 + 事件驱动** 架构：前端纯展示，所有 AI 逻辑、网络请求、文件操作在 Rust 后端完成，二者通过 Tauri IPC 通信。

```
┌──────────────────────────────────────────────────────┐
│                 Frontend (WebView)                    │
│         React 18 + TypeScript + TailwindCSS          │
│                                                       │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │ Sidebar │ │ ChatView │ │Composer  │ │ Settings │ │
│  │对话列表  │ │消息渲染   │ │输入发送   │ │ 模型配置  │ │
│  └─────────┘ └──────────┘ └──────────┘ └──────────┘ │
│  ┌────────────────┐ ┌──────────────────────────────┐│
│  │ MarkdownRenderer│ │ PluginManager / KnowledgeBase││
│  └────────────────┘ └──────────────────────────────┘│
│                        │                             │
│            ┌───────────┴───────────┐                 │
│            │  Tauri IPC Bridge     │                 │
│            │  invoke() + listen()  │                 │
│            └───────────┬───────────┘                 │
└────────────────────────│─────────────────────────────┘
                         │
┌────────────────────────│─────────────────────────────┐
│                  Rust Backend                         │
│                        │                              │
│  ┌─────────────────────┴──────────────────────────┐  │
│  │              commands (IPC 处理层)               │  │
│  │  conversation │ chat │ model │ plugin │ settings│  │
│  └──────┬──────────┬──────────┬──────────┬────────┘  │
│         │          │          │          │            │
│  ┌──────▼──┐ ┌─────▼────┐ ┌──▼──────┐ ┌─▼─────────┐  │
│  │ model-  │ │ context  │ │ tool-   │ │ plugin-   │  │
│  │ provider│ │ (上下文   │ │ registry│ │ engine    │  │
│  │ (模型   │ │  裁剪)    │ │ (工具   │ │ (WASM)    │  │
│  │  抽象)  │ │          │ │  调度)  │ │           │  │
│  └────┬────┘ └────┬─────┘ └────┬────┘ └────┬──────┘  │
│       │          │            │            │          │
│  ┌────▼──────────▼────────────▼────────────▼──────┐  │
│  │  conversation-store │ rag │ streaming │ security│  │
│  │     (SQLite 持久化 + 向量库 + 流式 + 加密)       │  │
│  └────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────┘
```

## 关键设计原则

### 1. 前端纯展示
所有 AI 逻辑、API 调用、文件 I/O 在 Rust 后端。前端只负责：
- 发送用户意图（`invoke` 命令）
- 接收并渲染流式更新（`listen` 事件）
- 本地 UI 状态管理

**好处：** API Key 永不进入前端；统一流式事件模型；后端可拦截/转换工具调用。

### 2. Provider 抽象
统一的 `ModelProvider` trait，云端 API 和本地模型实现相同接口。切换模型无需改对话逻辑。详见 [model-providers.md](model-providers.md)。

### 3. WASM 插件沙箱
自定义插件在 wasmtime 中运行，内存隔离 + 能力令牌权限控制。插件无法越权访问系统。详见 [plugin-development.md](plugin-development.md)。

### 4. API Key 不出后端
前端只传 provider ID，后端查库解密后使用，用完 `zeroize` 清零。详见 [security](#) 设计。

---

## Crate 划分

后端按职责拆分为多个 crate，便于测试与维护：

| Crate | 职责 | 主要依赖 |
|-------|------|----------|
| `core` | 共享类型（Message/Conversation/ToolDef）、配置、统一错误 | serde, thiserror |
| `commands` | Tauri IPC 命令处理器，编排各模块 | tauri, 所有其他 crate |
| `model-provider` | 模型抽象 trait + 各 Provider 实现 | reqwest, eventsource-stream |
| `conversation-store` | 对话/消息 CRUD + FTS5 全文搜索 | rusqlite, r2d2 |
| `tool-registry` | 工具注册、定义转换、调度执行 | tokio, serde_json |
| `plugin-engine` | WASM 插件加载/运行/权限 | wasmtime |
| `streaming` | SSE 解析 + 流式节流 + Tauri 事件桥接 | tokio-stream |
| `security` | API Key 加密/解密/密钥派生 | aes-gcm, argon2 |
| `context` | 上下文窗口管理、摘要、Token 预算 | tiktoken-rs |
| `rag` | 文档摄入、分块、向量检索、混合检索 | sqlite-vec, text-splitter |

**依赖方向：** `commands` 依赖所有业务 crate；业务 crate 依赖 `core`；`core` 不依赖任何业务 crate。

---

## 核心数据流

### 发送消息（流式 + 工具调用）

```
用户输入 → Composer
    │ invoke("send_message", { conversationId, content })
    ▼
commands::chat::send_message
    │ 1. conversation_store 加载对话 + 历史
    │ 2. context::builder 智能裁剪上下文（滑动窗口+摘要）
    │ 3. tool_registry 收集可用工具（内置+插件）
    │ 4. model_provider.chat_stream(api_key, request)
    ▼
Provider (reqwest POST + SSE)
    │ tokio Stream<StreamChunk>
    ▼
streaming::bridge
    │ StreamBuffer 节流（50ms/500char）
    │ emit("chat:stream-chunk", delta)
    ▼
Frontend: requestAnimationFrame 批量更新 → 虚拟列表渲染
    │
    │ [检测到 tool_call]
    ├─ emit("chat:tool-call") → 前端显示工具卡片
    │   [需审批?] → invoke("approve_tool") → 执行
    │   tool_registry.dispatch(name, args)
    ├─ emit("chat:tool-result")
    │   将结果回填到上下文，继续 chat_stream
    ▼
emit("chat:gen-complete", usage)
```

完整命令/事件协议见 [ipc.md](ipc.md)。

---

## 状态管理

### 后端状态（`AppState`，Tauri managed state）

```rust
pub struct AppState {
    pub db: DbPool,                          // SQLite 连接池
    pub providers: ProviderRegistry,        // 模型 Provider 注册表
    pub tools: ToolRegistry,                 // 工具注册表
    pub plugins: PluginEngine,               // 插件引擎
    pub key_manager: KeyManager,             // API Key 加密器
    pub active_streams: ActiveStreams,       // 进行中的流式请求（可取消）
    pub config: AppConfig,                   // 应用配置
}
```

### 前端状态（Zustand stores）

| Store | 职责 |
|-------|------|
| `conversationStore` | 对话列表、当前对话、消息、流式状态 |
| `modelStore` | Provider 列表、当前模型、可用模型 |
| `settingsStore` | 主题、字体、语言等设置 |
| `pluginStore` | 已安装插件、启用状态 |
| `knowledgeStore` | 知识库列表、索引状态 |

---

## 安全模型（Tauri v2 Capability）

Tauri v2 采用基于能力的权限系统。`src-tauri/capabilities/` 下分文件声明：

| 能力文件 | 范围 | 说明 |
|----------|------|------|
| `default.json` | 主窗口基础命令 | 对话/消息/设置等安全命令 |
| `shell.json` | shell_exec 工具 | 仅在用户审批后动态授予 |
| `filesystem.json` | file_read 工具 | 限定用户选择的路径 |
| `network.json` | 网络访问 | 限定 Provider API 域名 |

前端只能 `invoke` 已声明权限的命令；危险能力需运行时用户确认。
