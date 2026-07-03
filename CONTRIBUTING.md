# 开发规范

## Git 工作流

- `main`：稳定分支
- `feature/<name>`：功能分支
- `fix/<name>`：修复分支
- 提交信息：`<type>: <desc>`（feat / fix / perf / refactor / docs / test）

## Rust 规范

- edition 2021，stable toolchain
- 所有公共类型加文档注释
- 错误用 `thiserror` 定义 crate 级 `Error`，`?` 传播
- 异步用 `tokio`，IO 用 `reqwest` / `rusqlite`
- 公共类型 `Send + Sync`，共享状态用 `Arc<RwLock<T>>`
- 不用 `unwrap`/`expect`（测试除外），用 `?` 或显式处理
- `cargo fmt` + `cargo clippy -- -D warnings` 必须通过
- 新增 IPC 命令在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册

### 命令模块

`src-tauri/src/commands/` 下按领域分包（39+ 个命令）：

| 文件 | 职责 |
|------|------|
| `agents.rs` | Agent CRUD |
| `chat.rs` | send_message / stop_generation / regenerate / 工具调用循环 |
| `message.rs` | get_messages / search_messages / update_message / delete_messages_from |
| `conversation.rs` | 对话 CRUD |
| `rag_cmd.rs` | 知识库 CRUD + 文档导入/批量导入/预览/编辑/重命名/批量删除/搜索 |
| `plugins.rs` | 插件列表/配置/工具执行 |
| `settings.rs` | 键值设置读写 |
| `stats.rs` | 用量统计 |
| `export.rs` | 对话导入导出 |
| `log.rs` | 日志读写 |
| `tools.rs` | 内置工具定义（calculator + rag_search）|
| `test.rs` / `test_chat.rs` | 健康检查 / API 测试 |

## TypeScript 规范

- strict 模式，`noUnusedLocals: true`
- 类型与后端 serde 对齐（`src/types/index.ts`）
- 组件用函数组件 + hooks，副作用清晰隔离
- 性能敏感组件用 `React.memo` + 稳定回调（`useCallback`）

### 前端分层架构

**禁止跨层调用**。数据流单向：

```
组件 → hooks → stores → services → invoke()
```

| 层 | 目录 | 说明 |
|----|------|------|
| 组件 | `src/components/` | UI 渲染，通过 props 接收数据，通过 hooks 触发操作 |
| Hooks | `src/hooks/` | 逻辑封装，管理本地状态 + 调用 stores |
| Stores | `src/stores/` | Zustand 全局状态，调用 service 层 |
| Services | `src/services/` | Tauri `invoke()` 唯一出口，`invokeWithTimeout`（默认 8s 超时） |

### 组件约定

- 颜色用语义 token（`bg-background` / `text-foreground` / `bg-primary`），不用硬编码
- 图标用 `lucide-react`，不用 emoji
- 新增 UI 优先用 `src/components/ui/` 里的 shadcn 原语
- 路径别名 `@/*` → `src/*`

### 组件分类

| 目录 | 内容 |
|------|------|
| `ui/` | shadcn/ui 原语（button/input/textarea/select/dialog/tabs/tooltip/badge/card/skeleton/switch/sheet/avatar/popover/dropdown-menu/scroll-area/separator/label）|
| `layout/` | Sidebar / ChatHeader / ChatInputArea / ErrorBanner |
| `sidebar/` | AgentListView / ConversationListView / ConversationListItem / AgentEditorPanel |
| `chat/` | VirtualMessageList / MessageBubble / StreamingMessage / MessageSkeleton / EmptyChatPlaceholder |
| `settings/` | SettingsWindow（独立窗口）+ GeneralSettings / LogsPanel / KnowledgePanel / StatsPanel / PluginsPanel（均懒加载） |
| `common/` | IpcStatusIndicator / ModelSelector / MentionPopover / ContextMenu / ImagePreview |

## 命名

- Rust：snake_case（函数/变量）、PascalCase（类型/trait）
- TS：camelCase（变量/函数）、PascalCase（类型/组件）
- IPC 命令：snake_case；事件：kebab-case

## 测试与构建

```bash
# 前端
npx tsc --noEmit          # 类型检查
npm run build             # 生产构建（tsc + vite build，含 manualChunks 分包）

# 后端（在 src-tauri/ 下）
cargo check               # 快速检查
cargo build               # 编译
cargo test --workspace    # 全 workspace 测试
cargo clippy              # lint（建议 -D warnings）
```

- Rust：单元测试 `#[cfg(test)]` 模块（counter/chunking/buffer/store 等已有）
- 关键路径必测：流式渲染、上下文裁剪、RAG 检索、迁移、delete_from 安全性
- 改动后至少跑 `cargo test --workspace` + `npx tsc --noEmit`

## 性能红线

改动不得使以下退化：
- 1000 条消息对话流式 > 30fps
- 上下文裁剪后 token ≤ 模型上限
- 主 chunk gzip < 60KB（当前 47KB）
- RAG 检索 < 50ms（万级 chunk）

## 数据库

- 所有 DB 连接加超时：`get_timeout(5s)`（`db_conn!` 宏），不能用 `state.db.get()`（阻塞 tokio 线程）
- PRAGMA per-connection：`init_db` 用 `with_init` 对池中每条连接设 `foreign_keys=ON` 等
- 不跨 await 持有 DB 连接：embedding 等 network 调用前 drop 连接
- Schema 变更走版本化迁移（新增 `MIGRATION_NNN`），每个迁移 `unchecked_transaction` 包裹，不改已有表结构
- 数据放项目目录（`d:\AI\Ripple\ripple.db`），不存 AppData
- WAL 模式，foreign_keys=ON，busy_timeout=5000ms
