# Ripple 项目开发规范

## 数据持久化（最重要的要求）

- **所有数据放项目目录**：数据库 `ripple.db` 和日志 `logs/` 都在 `d:\AI\Ripple\` 下，不允许存到 `C:\Users\*\AppData\`
- **重启不丢数据**：对话、消息、工具卡片都必须持久化到 SQLite，重启发前端加载时要有完整历史
- **消息只存一次**：不要先插占位再插同 ID 的消息（主键冲突），流结束后一次性插入完整消息
- **每条消息都存 DB**：用户消息和 AI 回复都要写数据库，不能只在内存里

## 工具调用卡片

- **永久保留**：工具调用卡片是对话历史的一部分，必须永久显示，不能流结束就消失
- **位置正确**：卡片渲染在最后一条用户消息和 AI 回答之间
- **流式中可见**：流式输出过程中卡片就要显示，不等流结束

## 渲染与 UI

- **虚拟列表**：消息列表必须用 `@tanstack/react-virtual`，长对话只渲染可视区域
- **智能滚动**：用户上划看历史时，新消息不能强制把滚动拉到底部（只有靠近底部时才自动滚）
- **只渲染可见**：旧消息用 `React.memo` 缓存，流式时只有最后一条重渲染
- **工具卡片嵌入**：流式输出时工具卡片嵌入 AI 回答气泡内部，而不是单独的列表项

## 后端开发规范

- **所有 DB 连接加超时**：使用 `get_timeout(5s)`，不能用 `state.db.get()`（会阻塞 tokio 线程）
- **日志要够详细**：关键操作（send_message start / loaded / built / returning）打 INFO 日志，失败打 ERROR
- **API 兼容 OpenAI 格式**：只通过 newapi 之类的 OpenAI 兼容端点，不做私有协议适配
- **map_err 而非 unwrap**：命令函数中任何可能失败的操作都要 `?` 或 `map_err` 返回给前端
- **新增 IPC 命令**：在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册

## 前端开发规范

- **零未处理错误**：所有 `invoke` 调用要有 `try/catch`，后端报错在前端显示红色横幅
- **类型对齐后端**：TypeScript 类型定义与 Rust `serde` 序列化格式保持一致
- **不覆盖已有消息**：`switchConversation` 切换对话时，如果内存中已有该对话消息则不再从 DB 加载

### 前端架构（2026/07 重构后）

分层架构，禁止跨层调用：`组件 → hooks → stores → services → invoke()`。

| 目录 | 说明 |
|------|------|
| `src/lib/` | `utils.ts`（`cn()` 合并 Tailwind 类）、`constants.ts`（模型列表/快捷键/阈值） |
| `src/services/` | Tauri `invoke()` 唯一出口，按模块分包 11 个 service。`invoke.ts` 提供 `invokeWithTimeout`（8s 超时） |
| `src/stores/` | Zustand stores（chat/agent/settings/kb/ui）。`uiStore.ts` 管 UI 状态 |
| `src/hooks/` | 逻辑提取（useIpcStatus/useStreamEvents/useLogs/useStats/usePlugins/useSearch/useMentionCompletion/useTheme/useMediaQuery） |
| `src/components/ui/` | shadcn/ui 原语（17 个组件，基于 Radix + CSS 变量） |
| `src/components/layout/` | 布局骨架（Sidebar/ChatHeader/ChatInputArea/ErrorBanner） |
| `src/components/sidebar/` | 侧边栏（AgentListView/ConversationListView/ConversationListItem/AgentEditorPanel） |
| `src/components/chat/` | 聊天（VirtualMessageList/MessageBubble/StreamingMessage/MessageSkeleton/EmptyChatPlaceholder） |
| `src/components/settings/` | 设置面板（SettingsDialog + GeneralSettings/LogsPanel/KnowledgePanel/StatsPanel/PluginsPanel） |
| `src/components/common/` | 通用（IpcStatusIndicator/ModelSelector/MentionPopover） |
| `src/styles/globals.css` | 设计 token（CSS 变量 light/dark 两套，`darkMode: "class"`） |
| `src/types/` | TypeScript 类型（index.ts 主类型，theme.ts 主题枚举） |

约定：新增 UI 优先用 `@/components/ui/` 原语；颜色用语义 token（`bg-background`/`text-foreground`/`bg-primary`）不用硬编码 `slate-*`/`indigo-*`；图标用 `lucide-react` 不用 emoji；路径别名 `@/*` → `src/*`。

## Agent 系统

- **创建 Agent**：侧边栏 Agent 标签 → 点击虚线「新建 Agent」→ 选图标 → 输名称 → 创建
- **选中 Agent**：点击 Agent 后**不自动跳转** tab，右侧自动恢复该 Agent 的上次活跃会话（`lastActivePerAgent` 记录）
- **手动切换会话**：在「会话」tab 中点击切换，切换时自动记忆 Agent ↔ 会话对应关系
- **Agent 编辑**：侧边栏「编辑」标签，编辑后点保存（注意 `system_prompt` 需同时传 `system_prompt` 和 `systemPrompt` 两种命名，兼容 Tauri v2 的 camelCase/snake_case 转换）
- **{key} 占位符注入**：Agent 的 system prompt 中可用 `{键名}` 引用 `Agents/` 目录下的 `.txt` 文件内容
- **映射文件**：`Agents/agent_map.json` 定义 `{"键名": "文件名"}` 映射
- **每个 Agent 独立会话**：对话按 `agent_id` 过滤，互不干扰

## UI/UX 设计要求

- **设置面板**：可拖拽（标题栏 mousedown）、可缩放（CSS resize:both）、位置和大小存 localStorage。默认 820×620，居中偏上
- **设置面板关闭**：仅通过 Close 按钮关闭，点击遮罩层不关闭
- **侧边栏**：三标签（Agent/会话/编辑）。可折叠为图标模式（48px）。顶部 Logo + 底部全局设置 + IPC 状态灯
- **消息列表**：虚拟列表（@tanstack/react-virtual），仅渲染可视区域 + 上下 5 条缓冲
- **自动滚动**：仅用户靠近底部时自动滚（阈值 100px），上翻看历史不强制拉回
- **React.memo**：MessageBubble 缓存，旧消息不重渲染
- **@ 补全**：输入 @ 弹出知识库列表，↑↓ 选择 Enter 确认 Esc 关闭
- **工具卡片**：可折叠，lucide 图标，嵌入消息流，流式中可见，流结束保留
- **上下文裁剪配置**：Settings 面板可调（启用/最近消息数/摘要间隔/最大 Token）
- **Logs 面板**：每 3 秒自动刷新，保留滚动位置（仅底部才自动滚）
- **知识库**：文档以 4 列网格卡片展示，点击预览内容（Dialog），支持在线编辑后重新索引。删除 KB/文档需确认
- **主题切换**：Header 下拉切换浅色/深色/跟随系统，持久化到 localStorage
- **空状态**：未选对话时显示欢迎页 + 快捷键提示

## 插件系统

- **插件目录**：`plugins/插件名/manifest.json`
- **三种模式**：`mode` 字段 = `tool`（AI 调用）/ `transform`（消息处理）/ `daemon`（后台服务）
- **运行时**：`runtime` 字段 = `rhai`（安全沙箱）/ `node` / `python` / `shell`
- **工具注册**：插件 tools 自动合并到 `builtin_tools()`，AI 可见 `plugin_插件名:工具名`
- **权限**：非 rhai 运行时自动标记 `requires_approval: true`
- **配置**：`config_schema` 定义可编辑字段，UI 保存到 `config.json`
- **示例插件**：`plugins/hello/`（rhai 类型，带配置项）

## RAG 知识库

- **后端命令**：create/list/delete KB，import/list/delete/get_content/update_content Document，search
- **文档编辑**：`get_document_content` 拼接所有 chunk 返回全文 → 前端编辑 → `update_document_content` 删旧 chunk 重新分块嵌入
- **删除确认**：删除 KB 和文档均需确认弹窗（Dialog），提示不可恢复

## 沟通约定

- 需要增删改数据结构的改动，必须先确认对现有数据的影响
- 数据库 schema 变更走版本化迁移，不改已有表结构
