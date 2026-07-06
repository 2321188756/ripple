# Ripple 项目开发规范

## 数据持久化（最重要的要求）

- **所有数据放项目目录**：数据库 `ripple.db` 和日志 `logs/` 都在 `d:\AI\Ripple\` 下，不允许存到 `C:\Users\*\AppData\`
- **重启不丢数据**：对话、消息、工具卡片都必须持久化到 SQLite，重启发前端加载时要有完整历史
- **消息只存一次**：不要先插占位再插同 ID 的消息（主键冲突），流结束后一次性插入完整消息
- **每条消息都存 DB**：用户消息和 AI 回复都要写数据库，不能只在内存里

## 工具调用卡片

- **永久保留**：工具调用卡片是对话历史的一部分，必须永久显示，不能流结束就消失
- **按轮次显示**：卡片插入到最后一条 user 消息之后（当前轮次），`sendMessage` 时清空上一轮 `toolEvents`，避免跨轮次累积堆错位置
- **流式中可见**：流式输出过程中卡片就要显示，不等流结束

## 渲染与 UI

- **虚拟列表**：消息列表必须用 `@tanstack/react-virtual`，长对话只渲染可视区域
- **智能滚动**：用户上划看历史时，新消息不能强制把滚动拉到底部（只有靠近底部时才自动滚）
- **React.memo**：MessageBubble memo 缓存，接收原始 `content` blocks 内部 useMemo 提取 text/images（不要在父组件 `.filter().map()` 产生新数组击穿 memo）；MarkdownRenderer 也 memo
- **Mermaid 不突变 DOM**：用 `MermaidBlock` 组件 + state 承载 svg，禁止 `el.innerHTML/outerHTML` 命令式操作（会破坏 React reconciliation）
- **原子 selector**：App/Sidebar/ChatHeader 用 `useStore((s) => s.field)` 精确订阅，**不要**整 store 解构订阅 `streamingText`（每 token 变化会全树重渲染）。流式文本由 VirtualMessageList 自行订阅

## 后端开发规范

- **所有 DB 连接加超时**：使用 `get_timeout(5s)`（`db_conn!` 宏），不能用 `state.db.get()`（会阻塞 tokio 线程）
- **不要跨 await 持有 DB 连接**：embedding 等 network 调用前 drop 连接，调用后再重新获取，避免 8 连接池耗尽
- **PRAGMA per-connection**：`init_db` 用 `with_init` 对池中每条连接设 `foreign_keys=ON` 等，不能只设第一条
- **迁移事务化**：`run_migrations` 每个迁移用 `unchecked_transaction` 包裹，失败整体回滚；ALTER TABLE 不幂等，靠事务保证不卡死
- **delete_from 必须校验存在**：`delete_from` 先查 `from_message_id` 的 `created_at`，不存在返回 `NotFound`，禁止 COALESCE 回退 1970（会删光整段对话）
- **日志要够详细**：关键操作打 INFO 日志，失败打 ERROR
- **API 兼容 OpenAI 格式**：只通过 newapi 之类的 OpenAI 兼容端点，不做私有协议适配
- **map_err 而非 unwrap**：命令函数中任何可能失败的操作都要 `?` 或 `map_err` 返回给前端；不要 `let _ =` 吞掉删除/写入错误
- **新增 IPC 命令**：在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册

## 流式与取消

- **stop_generation 必须真取消**：`ActiveStream` 带 `cancel: Arc<Notify>` + `cancelled: Arc<AtomicBool>`；`chat_with_tools` 用 `tokio::select!` 在 `consume_stream` 与 `cancel.notified()` 间竞速；循环顶检查 `cancelled` 锁存标志
- **工具循环有上限**：`MAX_TOOL_ROUNDS = 8`，防止模型反复 tool call 死循环
- **流错误不静默**：`StreamEvent::Error` 设 `had_error`，break 后发 `chat:gen-error` 并返回部分文本（不要当成功返回截断文本）

## 前端开发规范

- **零未处理错误**：所有 `invoke` 调用要有 `try/catch`，后端报错在前端显示红色横幅
- **类型对齐后端**：TypeScript 类型定义与 Rust `serde` 序列化格式保持一致（`src/types/index.ts`）
- **switchConversation 始终刷新**：切换对话始终后台 `messageService.list` 重新加载消息，不命中缓存就 return（否则切回看不到后端新落库的回复）
- **流式中切对话先 stop**：`switchConversation` 检测到 `streamingText !== null` 时先 `await stopGeneration()` 再切（必须在改 activeId 之前，否则停错对话）
- **首块竞态**：`sendMessage`/`regenerate` 在 await 前先置 `streamingText: ""`，`appendToStreaming` 在 `streamingMsgId===null && streamingText===""` 时用首块 message_id 锁存，避免开头丢字
- **finalize 用事件对话 id**：`finalizeStreaming`/`handleStreamError` 用 `payload.conversation_id` 落库（不是 activeId），流式中切走也能落到原对话
- **stop 保留部分回复**：`stopGeneration` 把已生成的 `streamingText` 存为助手消息，不直接丢弃

### 前端架构（分层禁止跨层）

```
组件 → hooks → stores → services → invoke()
```

| 目录 | 说明 |
|------|------|
| `src/lib/` | `utils.ts`（`cn()`）、`constants.ts`（模型列表/快捷键/阈值）、`openSettings.ts`（打开独立设置窗口） |
| `src/services/` | Tauri `invoke()` 唯一出口，`invokeWithTimeout`（8s 超时） |
| `src/stores/` | Zustand stores（chat/agent/settings/kb/ui） |
| `src/hooks/` | 逻辑提取（useIpcStatus/useStreamEvents/useLogs/useStats/usePlugins/useSearch/useMentionCompletion/useTheme/useMediaQuery） |
| `src/components/ui/` | shadcn/ui 原语（基于 Radix + CSS 变量） |
| `src/components/layout/` | 布局骨架（Sidebar/ChatHeader/ChatInputArea/ErrorBanner） |
| `src/components/sidebar/` | 侧边栏（AgentListView/ConversationListView/ConversationListItem/AgentEditorPanel） |
| `src/components/chat/` | 聊天（VirtualMessageList/MessageBubble/StreamingMessage/MessageSkeleton/EmptyChatPlaceholder） |
| `src/components/settings/` | SettingsWindow（独立窗口）+ GeneralSettings/LogsPanel/KnowledgePanel/StatsPanel/PluginsPanel（均懒加载） |
| `src/components/common/` | 通用（IpcStatusIndicator/ModelSelector/MentionPopover/ContextMenu/ImagePreview） |
| `src/styles/globals.css` | 设计 token（CSS 变量 light/dark 两套，`darkMode: "class"`，`useTheme` 切换并持久化） |
| `src/types/` | TypeScript 类型（index.ts + theme.ts） |

约定：新增 UI 优先用 `@/components/ui/` 原语；颜色用语义 token（`bg-background`/`text-foreground`/`bg-primary`）不用硬编码颜色；图标用 `lucide-react` 不用 emoji；路径别名 `@/*` → `src/*`。

## 多窗口（设置窗口）

- 设置是**独立 OS 窗口**：`openSettingsWindow()` 用 `WebviewWindow` 创建，加载 `index.html#settings`
- `main.tsx` 按 hash 路由：`#settings` → `<SettingsWindow/>`，否则 `<App/>`（App 懒加载，设置窗口不加载聊天 bundle）
- 跨窗口同步：两窗口是独立 JS 上下文，store 不共享。SettingsWindow 在 settingsStore/kbStore 变化时 `emit("ripple:settings-changed")`；App `listen` 后 reload settings + KB
- 标题栏用 `data-tauri-drag-region` 让系统原生拖动
- capabilities 需 `core:webview:allow-create-webview-window` / `core:window:allow-close` / `allow-set-focus`，且 `"windows": ["main", "settings"]`

## Agent 系统

- **创建 Agent**：侧边栏 Agent 标签 → 点击虚线「新建 Agent」→ 选图标 → 输名称 → 创建
- **选中 Agent**：点击 Agent 后**不自动跳转** tab，右侧自动恢复该 Agent 的上次活跃会话（`lastActivePerAgent` 记录）
- **无会话 Agent**：`restoreLastActive` 在该 Agent 无会话时**清空 activeId**（显示空状态），不残留别的 Agent 的对话；用户发消息时 `sendMessage` 自动用当前 Agent id 建会话
- **手动切换会话**：在「会话」tab 中点击切换，自动记忆 Agent ↔ 会话对应关系
- **Agent 编辑**：侧边栏「编辑」标签（注意 `system_prompt` 需同时传 `system_prompt` 和 `systemPrompt` 两种命名，兼容 Tauri v2 的 camelCase/snake_case 转换）
- **{key} 占位符注入**：Agent 的 system prompt 中可用 `{键名}` 引用 `Agents/` 目录下的 `.txt` 文件内容
- **每个 Agent 独立会话**：对话按 `metadata.agent_id` 过滤（`list_conversations` 的 `metadata LIKE` 查询），互不干扰

## 插件系统

- **插件目录**：`plugins/插件名/manifest.json` + 入口代码文件
- **三种模式**：`mode` = tool（AI 调用）/ transform（消息处理）/ daemon（后台服务）
- **运行时**：`runtime` = rhai（沙箱，目前返回脚本内容）/ node / python / shell（后三者 `std::process::Command` 子进程）
- **工具注册**：`plugin_tools()` 注册名为 `plugin_{name}:{tool}`，`source: ToolSource::Plugin`
- **工具执行**：`exec_by_tool_name` 必须先**剥离 `plugin_` 前缀**再按 `:` 拆分查注册表（注册表以 `{name}` 为键）
- **权限**：非 rhai 运行时 `requires_approval: true`
- **配置**：`config_schema` 定义可编辑字段，UI 保存到 `plugins/插件名/config.json`

## RAG 知识库

- **嵌入存储**：向量以 JSON 存 `chunks.embedding_json` 列（**不使用 sqlite-vec**），检索时 Rust 端 `cosine_similarity` 暴力计算
- **混合检索**：向量 KNN + FTS5 BM25（`chunks_fts`，由 MIGRATION_005 触发器维护）+ RRF 融合
- **RRF 用排名位置**：FTS5 rank 是负数，`as usize` 会饱和为 0；必须用结果排名位置（0,1,2,...）参与 RRF
- **import_folder 容错**：嵌入批次失败时**中止整篇文档**并标 `error`，禁止 `continue` 跳过（会导致 zip 错配存错向量）
- **文档编辑**：`get_document_content` 拼接所有 chunk 返回全文 → 前端编辑 → `update_document_content` 删旧 chunk 重新分块嵌入
- **批量导入**：`import_folder` 递归扫描目录，支持 txt/md/pdf/rs/py/js/ts
- **删除确认**：删 KB / 删文档 / 批量删均需确认弹窗

## IPC 命令一览（40+ 个）

| 模块 | 命令 |
|------|------|
| agents | list_agents, create_agent, update_agent, delete_agent, get_agent |
| rag_cmd | create_kb, list_kbs, delete_kb, list_docs, import_document, search_kb, delete_document, get_document_content, update_document_content, import_folder, batch_delete_documents, rename_document |
| chat | send_message, stop_generation, regenerate |
| message | get_messages, search_messages, update_message, delete_messages_from |
| conversation | create_conversation, list_conversations, delete_conversation, get_conversation, update_conversation |
| export | export_conversation, import_conversation |
| memory | reindex_memories, list_memory_files, list_all_memory_files, get_memory_file, save_memory_file, delete_memory_file, delete_agent_memory_file, memory_stats, generate_memory_tags, open_memory_dir |
| settings | get_setting, set_setting |
| plugins | list_plugins, toggle_plugin, execute_plugin_tool, get_plugin_config, set_plugin_config, approve_tool_call, get_agent_permission_level, set_agent_permission_level, list_trusted_tools, revoke_trust |
| stats | get_usage_stats |
| log | log_event, get_log_path, get_logs |
| test | ping, test_chat |

## 沟通约定

- 需要增删改数据结构的改动，必须先确认对现有数据的影响
- 数据库 schema 变更走版本化迁移（新增 MIGRATION_NNN），不改已有表结构
