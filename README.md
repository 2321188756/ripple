# Ripple · 涟漪

> 跨平台桌面 AI 聊天助手 —— 融合多家大模型、工具调用、自定义 Agent、RAG 知识库与插件系统。

涟漪（Ripple）寓意 AI 的回答如水波层层扩散：从一个问题出发，调动模型、工具、知识库，最终泛起完整而连贯的回应。

---

## ✨ 功能

| 功能 | 说明 |
|------|------|
| 💬 **多对话管理** | 搜索(FTS5) / 重命名 / 删除 / 置顶，每个 Agent 独立会话列表 |
| 🌐 **多模型支持** | newapi/OpenAI 兼容端点，Header 下拉切换模型 |
| 📝 **富文本渲染** | Markdown + 代码高亮(Prism) + 表格 + KaTeX 数学公式 + Mermaid 图表 |
| 🔧 **工具调用** | 计算器 + RAG 搜索，ToolCall 卡片嵌入流式气泡，永久保留 |
| 🤖 **自定义 Agent** | 多 Agent 管理 + 可选图标 + {key} 文件注入 + 会话自动恢复 |
| 📚 **RAG 知识库** | 文档分块/Embedding/混合检索 + @ 注入。网格文档卡片，支持预览和在线编辑后重新索引 |
| 🧩 **插件系统** | tool/transform/daemon 三种模式，Python/JS/Rhai/Shell 运行时，配置在线编辑 |
| ⚡ **性能优化** | 虚拟列表 + React.memo + StreamBuffer 节流 + 上下文裁剪(可配置) |
| 🎨 **主题切换** | 浅色 / 深色 / 跟随系统，Header 一键切换，持久化 |
| 🔒 **数据本地化** | SQLite 持久化（`ripple.db`），重启不丢 |
| 📋 **日志/统计** | 文件日志 + 应用内查看 + Token 用量可视化面板 |
| ⌨️ **快捷键** | Ctrl+N 新对话 / Ctrl+K 搜索 / Ctrl+, 设置 |
| 📥 **导出** | 对话导出为 Markdown 文件 |

## 🏗️ 技术栈

| 层 | 技术 |
|----|------|
| **桌面框架** | Tauri v2 |
| **后端** | Rust（tokio, 7 个 crate, sqlite-vec 向量库） |
| **前端** | React 18 + TypeScript + Vite 6 |
| **UI 组件库** | shadcn/ui（Radix 原语）+ TailwindCSS v3.4 + CSS 变量设计 token |
| **状态管理** | Zustand v5 |
| **虚拟滚动** | @tanstack/react-virtual |
| **图标** | lucide-react |
| **动画** | framer-motion |
| **数据库** | SQLite（rusqlite + FTS5） |

## 📁 项目结构

```
Ripple/
├── src/                          # React 前端（分层架构）
│   ├── components/
│   │   ├── ui/                   # shadcn/ui 原语（17 个组件）
│   │   ├── layout/               # 布局骨架（Sidebar/ChatHeader/ChatInputArea/ErrorBanner）
│   │   ├── sidebar/              # 侧边栏（AgentList/ConversationList/AgentEditor）
│   │   ├── chat/                 # 聊天（VirtualMessageList/MessageBubble/StreamingMessage）
│   │   ├── settings/             # 设置面板（General/Logs/Knowledge/Stats/Plugins）
│   │   └── common/               # 通用（IpcStatus/ModelSelector/MentionPopover）
│   ├── hooks/                    # 自定义 hooks（9 个）
│   ├── services/                 # IPC 封装层（11 个 service，invoke 唯一出口）
│   ├── stores/                   # Zustand 状态（chat/agent/settings/kb/ui）
│   ├── lib/                      # 工具（cn() + 常量）
│   ├── types/                    # TypeScript 类型定义
│   └── styles/                   # globals.css（CSS 变量设计 token）
│
├── src-tauri/                    # Rust 后端
│   ├── crates/                   # 7 个核心 crate
│   ├── src/commands/             # IPC 命令模块（含 RAG 文档 CRUD）
│   └── capabilities/             # Tauri v2 权限
│
├── Agents/                       # Agent 设定文件 + agent_map.json
├── plugins/                      # 插件（manifest.json + 代码文件）
└── logs/                         # 运行日志
```

## 🚀 启动

```bash
npm run tauri dev    # 开发模式（含 Rust 编译）
npm run dev           # 仅前端预览（无后端）
start.bat             # Windows 一键启动
```

## ⚙️ 配置

Settings 面板（可拖拽、可缩放、位置/大小持久化到 localStorage）：

- **通用** — API Key / Base URL / 模型 + 上下文裁剪参数
- **知识库** — 创建/删除 KB，导入文档（网格卡片），点击预览内容，支持编辑后重新索引
- **插件** — 列表/配置编辑
- **统计** — Token 用量可视化（每日趋势/角色分布/热门模型）
- **日志** — 实时日志查看（3s 自动刷新）
