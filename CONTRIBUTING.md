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
- 公共类型 `Send + Sync`，共享状态用 `Arc<RwLock<T>>`（`parking_lot`）
- 不用 `unwrap`/`expect`（测试除外），用 `?` 或显式处理
- `cargo fmt` + `cargo clippy -- -D warnings` 必须通过
- 新增 IPC 命令在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册

### 命令模块

`src-tauri/src/commands/` 下按领域分包：

| 文件 | 职责 |
|------|------|
| `agents.rs` | Agent CRUD |
| `chat.rs` | 消息发送 / 流式生成 / 工具调用 |
| `conversation.rs` | 对话管理 |
| `message.rs` | 消息查询 / 搜索 |
| `rag_cmd.rs` | 知识库 CRUD + 文档导入/预览/编辑/搜索 |
| `plugins.rs` | 插件列表/配置/工具执行 |
| `settings.rs` | 键值设置读写 |
| `stats.rs` | 用量统计 |
| `export.rs` | 对话导入导出 |
| `log.rs` | 日志读写 |
| `test.rs` / `test_chat.rs` | 健康检查 / API 测试 |
| `tools.rs` | 内置工具定义 |

## TypeScript 规范

- strict 模式，`noUnusedLocals: true`
- 类型与后端 serde 对齐（`src/types/index.ts`）
- 组件用函数组件 + hooks，副作用清晰隔离
- 性能敏感组件用 `React.memo` + 稳定回调（`useCallback`）

### 前端分层架构

**禁止跨层调用**。数据流单向：`组件 → hooks → stores → services → invoke()`

| 层 | 目录 | 说明 |
|----|------|------|
| 组件 | `src/components/` | UI 渲染，通过 props 接收数据，通过 hooks 触发操作 |
| Hooks | `src/hooks/` | 逻辑封装，管理本地状态 + 调用 stores |
| Stores | `src/stores/` | Zustand 全局状态，调用 service 层 |
| Services | `src/services/` | Tauri `invoke()` 唯一出口，提供 `invokeWithTimeout`（默认 8s 超时） |

### 组件约定

- 颜色用语义 token（`bg-background` / `text-foreground` / `bg-primary`），不用硬编码 `slate-*` / `indigo-*`
- 图标用 `lucide-react`，不用 emoji
- 新增 UI 优先用 `src/components/ui/` 里的 shadcn 原语
- 路径别名 `@/*` → `src/*`

## 命名

- Rust：snake_case（函数/变量）、PascalCase（类型/trait）
- TS：camelCase（变量/函数）、PascalCase（类型/组件）
- IPC 命令：snake_case；事件：kebab-case

## 测试

- Rust：单元测试 `#[cfg(test)]` 模块 + 集成测试 `tests/`
- 前端：Vitest 单元 + Playwright E2E
- 关键路径必测：流式渲染、上下文裁剪、RAG 检索、工具审批

## 性能红线

改动不得使以下退化：
- 1000 条消息对话流式 > 30fps
- 上下文裁剪后 token ≤ 模型上限
- RAG 检索 < 50ms（10 万 chunk）

## 数据库

- 所有 DB 连接加超时：`get_timeout(5s)`，不能用 `state.db.get()`（阻塞 tokio 线程）
- Schema 变更走版本化迁移，不改已有表结构
- 数据放项目目录（`d:\AI\Ripple\ripple.db`），不存 AppData
