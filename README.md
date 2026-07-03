# Ripple · 涟漪

> 跨平台桌面 AI 聊天助手 —— 融合多家大模型、工具调用、自定义 Agent、RAG 知识库与插件系统。

涟漪（Ripple）寓意 AI 的回答如水波层层扩散：从一个问题出发，调动模型、工具、知识库，最终泛起完整而连贯的回应。

---

## ✨ 功能

| 功能 | 说明 |
|------|------|
| 💬 **多对话管理** | 搜索(FTS5) / 重命名 / 删除 / 置顶 / 消息编辑/重生成/删除，每个 Agent 独立会话 |
| 🌐 **多模型支持** | newapi / OpenAI 兼容端点，Header 下拉切换模型 |
| 📝 **富文本渲染** | Markdown + 代码高亮(PrismLight 按需注册) + 表格 + KaTeX 数学公式 + Mermaid 图表 + 图片渲染 |
| 🔧 **工具调用** | 计算器 + RAG 搜索，ToolCall 卡片嵌入流式气泡，按轮次显示、永久保留 |
| 🤖 **自定义 Agent** | 多 Agent 管理 + 可选图标 + {key} 文件注入 + 会话自动恢复；切到无会话的 Agent 自动清空右侧 |
| 📚 **RAG 知识库** | 文档分块/Embedding/混合检索(向量+FTS5+RRF) + @ 注入。网格文档卡片 + 预览 + 在线编辑后重新索引 + 批量导入文件夹/批量删除/重命名 |
| 🧩 **插件系统** | tool/transform/daemon 三种模式，rhai/node/python/shell 运行时，配置在线编辑 |
| 🖼️ **多模态图片** | 拖拽/粘贴/选择图片上传，输入区缩略图预览，消息内渲染，点击全屏缩放预览 |
| ✏️ **消息编辑** | 右键菜单编辑/重生成/删除消息，编辑后自动触发重生成 |
| 🎨 **主题切换** | 浅色 / 深色 / 跟随系统，Header 一键切换，持久化 |
| ⚡ **性能优化** | 虚拟列表 + 原子 selector + React.memo + StreamBuffer 节流 + 上下文裁剪(可配置) + Vite 分包 |
| 🪟 **独立设置窗口** | 设置作为独立 OS 窗口打开（原生拖动/缩放/置顶），跨窗口状态自动同步 |
| 🔒 **数据本地化** | SQLite 持久化（`ripple.db`），重启不丢 |
| 📋 **日志/统计** | 文件日志 + 应用内查看 + Token 用量可视化面板 |
| ⌨️ **快捷键** | Ctrl+N 新对话 / Ctrl+K 搜索 / Ctrl+, 设置 |
| 📥 **导出** | 对话导出为 Markdown 文件 |

## 🏗️ 技术栈

| 层 | 技术 |
|----|------|
| **桌面框架** | Tauri v2 |
| **后端** | Rust（tokio, 7 个 crate, FTS5 全文检索, 嵌入向量存 SQLite） |
| **前端** | React 18 + TypeScript + Vite 6 |
| **UI 组件库** | shadcn/ui（Radix 原语）+ TailwindCSS v3.4 + CSS 变量设计 token |
| **状态管理** | Zustand v5（原子 selector 精确订阅） |
| **虚拟滚动** | @tanstack/react-virtual |
| **图标** | lucide-react |
| **数据库** | SQLite（rusqlite + r2d2 连接池 + FTS5） |

## 📁 项目结构

```
Ripple/
├── src/                          # React 前端（分层架构）
│   ├── components/
│   │   ├── ui/                   # shadcn/ui 原语
│   │   ├── layout/               # 布局骨架（Sidebar/ChatHeader/ChatInputArea/ErrorBanner）
│   │   ├── sidebar/              # 侧边栏（AgentList/ConversationList/AgentEditor）
│   │   ├── chat/                 # 聊天（VirtualMessageList/MessageBubble/StreamingMessage）
│   │   ├── settings/             # 设置（SettingsWindow + General/Logs/Knowledge/Stats/Plugins 面板）
│   │   └── common/               # 通用（IpcStatus/ModelSelector/MentionPopover/ContextMenu/ImagePreview）
│   ├── hooks/                    # 自定义 hooks
│   ├── services/                 # IPC 封装层（invoke 唯一出口）
│   ├── stores/                   # Zustand 状态（chat/agent/settings/kb/ui）
│   ├── lib/                      # 工具（cn() + 常量 + openSettings）
│   ├── types/                    # TypeScript 类型定义
│   └── styles/                   # globals.css（CSS 变量设计 token）
│
├── src-tauri/                    # Rust 后端
│   ├── crates/                   # 7 个核心 crate
│   ├── src/commands/             # IPC 命令模块（40+ 命令）
│   └── capabilities/             # Tauri v2 权限（main + settings 窗口）
│
├── Agents/                       # Agent 设定文件 + agent_map.json
├── plugins/                      # 插件（manifest.json + 代码文件）
└── logs/                         # 运行日志
```

## 🚀 启动

```bash
npm run tauri dev    # 开发模式（含 Rust 编译 + Vite HMR）
npm run dev          # 仅前端预览（无后端）
npm run build        # 前端生产构建（tsc + vite build）
cargo test --workspace   # 后端测试（在 src-tauri/ 下）
```

## ⚙️ 设置（独立窗口）

点侧边栏「全局设置」或按 `Ctrl+,` 打开**独立设置窗口**（OS 原生窗口，可拖出主窗口、独立最小化/置顶，Escape 关闭）。设置窗口加载 `index.html#settings`，前端按 hash 路由只渲染设置界面，不加载聊天主 bundle，打开轻快。设置/知识库改动会通过 `ripple:settings-changed` 事件通知主窗口刷新缓存。

- **通用** — API Key / Base URL / 模型 + 上下文裁剪参数
- **知识库** — 创建/删除 KB，导入文件/文件夹，网格卡片展示，批量选择删除，双击重命名，右键菜单（打开编辑/预览/重命名/属性/删除），点击预览内容
- **插件** — 列表/配置编辑
- **统计** — Token 用量可视化（每日趋势/角色分布/热门模型）
- **日志** — 实时日志查看

## ⌨️ 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+N` | 新建对话 |
| `Ctrl+K` | 搜索消息 |
| `Ctrl+,` | 打开设置窗口 |
| 消息右键 | 编辑 / 重生成 / 删除 |
| 文档右键 | 打开编辑 / 打开预览 / 重命名 / 属性 / 删除 |

## 📚 文档

详见 [docs/](docs/)：架构、技术栈、数据库、IPC 协议、性能、RAG、模型抽象、插件开发、路线图。
