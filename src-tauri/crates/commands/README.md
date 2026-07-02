# crate: commands

Tauri IPC 命令处理器层。前端 `invoke()` 的入口，编排各业务 crate 完成请求。

## 职责

- 声明 `#[tauri::command]` 函数，接收前端调用
- 编排业务逻辑：加载上下文 → 调用 provider → 处理流式 → 发事件
- 持有 `AppState`（DB 池、provider 注册表、工具注册表、插件引擎、key manager）
- 错误转换为前端可读字符串

## 模块

| 文件 | 命令域 |
|------|--------|
| `conversation.rs` | 对话 CRUD |
| `message.rs` | 消息列表（分页） |
| `chat.rs` | send_message / stop / regenerate / approve_tool_call（最复杂，编排完整聊天流） |
| `model.rs` | provider 增删查、连接测试、拉取模型、切换 |
| `plugin.rs` | 插件安装/卸载/启用/配置 |
| `settings.rs` | 设置读写 |
| `search.rs` | 对话/消息全文搜索 |
| `knowledge.rs` | 知识库 CRUD + 文档导入 + 检索 |

## 注册

在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 中统一注册所有命令。

协议详见 [docs/ipc.md](../../../docs/ipc.md)。
