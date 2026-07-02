# crate: tool-registry

工具注册与调度系统。管理内置工具与插件工具，供 AI 在对话中调用。

## 职责

- `ToolRegistry`：注册工具定义（`ToolDefinition`），按名查找
- 工具定义统一格式，按 Provider 转换为各家 API schema（委托 model-provider）
- 工具调度执行：接收 AI 的 tool_call → 路由到对应实现 → 返回结果
- 危险工具的审批门控：`requires_approval` 标记的工具需前端确认后才执行
- 执行结果写入审计日志

## 工具来源

| 来源 | 说明 |
|------|------|
| `Builtin` | Rust 原生实现（web_search / file_read / shell_exec / calculator / rag_search） |
| `Plugin` | 来自 plugin-engine 的 WASM 插件 |
| `UserDefined` | 用户在设置中自定义（如调用某 HTTP 端点） |

## 执行流程

```
AI tool_call(name, args)
  ↓
registry.dispatch(name, args)
  ↓
[requires_approval?] → 挂起，emit chat:tool-call → 等待 approve_tool_call
  ↓
执行（builtin / plugin / user-defined）
  ↓
emit chat:tool-result
  ↓
写 tool_audit_log
  ↓
结果回填上下文，继续生成
```

内置工具实现在 plugin-engine 的 `builtin/` 下（原生实现，非 WASM）。
