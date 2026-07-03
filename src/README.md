# Frontend (src/)

React 18 + TypeScript + Vite + TailwindCSS。**纯展示层**：所有 AI 逻辑在 Rust 后端，前端经 Tauri `invoke`/`listen` 通信。

## 入口与路由

`main.tsx` 按 `window.location.hash` 路由：

- 无 hash → 主窗口：`<App/>`（**lazy 加载**，聊天主 bundle 不进设置窗口）
- `#settings` → 独立设置窗口：`<SettingsWindow/>`（由 `openSettingsWindow()` 创建的 `WebviewWindow` 加载）

## 目录

```
src/
├── main.tsx                  # 入口 + hash 路由
├── App.tsx                   # 主窗口骨架（懒加载）
├── components/
│   ├── MarkdownRenderer/     # Markdown + PrismLight 代码高亮 + KaTeX + MermaidBlock（memo）
│   ├── ToolCallCard/         # 可折叠工具调用卡片
│   ├── ui/                   # shadcn/ui 原语（Radix + CSS 变量）
│   ├── layout/               # Sidebar / ChatHeader / ChatInputArea / ErrorBanner
│   ├── sidebar/              # AgentListView / ConversationListView / ConversationListItem / AgentEditorPanel
│   ├── chat/                 # VirtualMessageList / MessageBubble / StreamingMessage / MessageSkeleton / EmptyChatPlaceholder
│   ├── settings/             # SettingsWindow（独立窗口）+ General/Logs/Knowledge/Stats/Plugins 面板（懒加载）
│   └── common/               # IpcStatusIndicator / ModelSelector / MentionPopover / ContextMenu / ImagePreview
├── hooks/                    # useIpcStatus / useStreamEvents / useLogs / useStats / usePlugins / useSearch / useMentionCompletion / useTheme / useMediaQuery
├── services/                 # Tauri invoke 唯一出口（invokeWithTimeout 8s）
├── stores/                   # chatStore / agentStore / settingsStore / kbStore / uiStore（Zustand）
├── lib/                      # utils(cn) / constants / openSettings（打开独立设置窗口）
├── types/                    # IPC 类型（与后端 serde 对齐）
└── styles/                   # globals.css（CSS 变量设计 token，light/dark）
```

## 订阅约定（性能关键）

- **原子 selector**：`useStore((s) => s.field)` 精确订阅单个字段，禁止 `const { ... } = useStore()` 整 store 解构
- **`streamingText` 每 token 变化**，只由 `VirtualMessageList` 订阅；App/Sidebar/ChatHeader 不订阅，避免每 token 全树重渲染
- App 只订阅布尔派生量：`streaming = useChatStore((s) => s.streamingText !== null)`、`hasMessages = useChatStore((s) => ...)`

## 性能

- **虚拟列表**：`@tanstack/react-virtual`，只渲染可视区 + overscan 5，`measureElement` 动态高度
- **React.memo**：`MessageBubble`（接收 `content` blocks 内部 useMemo 提取 text/images）、`MarkdownRenderer`、`ConversationListItem`
- **智能滚动**：`onScroll` 函数式更新，仅靠近底部时自动滚
- **流式节流**：后端 StreamBuffer（50ms/500 字符）节流后，前端 store 直接 append
- **Vite 分包**：manualChunks（react-vendor / radix-vendor / markdown / syntax-highlight）+ target esnext，主 chunk 161KB
- **懒加载**：`App` 懒加载（设置窗口不加载聊天 bundle）；设置面板组件 lazy

## 跨窗口同步

设置窗口与主窗口是独立 JS 上下文，store 不共享。`SettingsWindow` 在 settingsStore/kbStore 变化时 `emit("ripple:settings-changed")`；`App` `listen` 后 reload settings + KB。

## IPC

所有后端调用通过 `src/services/` 的 `invokeWithTimeout` 完成，类型定义在 `types/`。流式事件由 `useStreamEvents` hook 在 App 顶层统一注册，转发到 `chatStore`：

- `chat:stream-chunk` → `appendToStreaming`（首块锁存 message_id 防竞态）
- `chat:gen-complete` → `finalizeStreaming`（用 payload.conversation_id 落库）
- `chat:gen-error` → `handleStreamError`（保留部分文本 + 清流 + 设错误）
- `chat:tool-call` → `addToolEvent`
