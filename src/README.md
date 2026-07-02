# Frontend (src/)

React 18 + TypeScript + Vite + TailwindCSS。**纯展示层**：所有 AI 逻辑在 Rust 后端，前端经 Tauri `invoke`/`listen` 通信。

## 目录

```
src/
├── components/              # UI 组件
│   ├── MarkdownRenderer/    # Markdown + 代码高亮(Prism) + 表格 + 数学公式(KaTeX)
│   └── ToolCallCard/        # 可折叠工具调用卡片（名称/状态/输入/输出）
├── stores/                  # Zustand 状态
│   ├── chatStore.ts         # 对话/消息/流式/工具事件
│   └── settingsStore.ts     # API Key/Base URL/默认模型（持久化到 DB）
├── types/                   # IPC 类型定义（与后端 serde 对齐）
└── styles/                  # 全局样式
```

## 性能

- **虚拟列表**：`@tanstack/react-virtual`，只渲染可视区域 + 上下 5 条缓冲
- **React.memo**：`MessageBubble` 缓存，旧消息不重渲染
- **智能滚动**：仅用户靠近底部时自动滚，上翻看历史不打断
- **流式节流**：后端 StreamBuffer 节流后，前端 requestAnimationFrame 批量更新

## IPC

所有后端调用通过 `@tauri-apps/api/core` 的 `invoke`/`listen` 完成，类型定义在 `types/`。
