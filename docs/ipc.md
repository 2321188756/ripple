# IPC 协议（Tauri 命令与事件）

前端通过 `invoke()` 调用后端命令，通过 `listen()` 接收后端事件。命名约定：

- **命令**：`<verb>_<noun>` 或 `<noun>_<verb>`，snake_case（如 `create_conversation`、`send_message`、`list_conversations`）
- **事件**：`<domain>:<event>`，kebab-case（如 `chat:stream-chunk`）
- 所有命令返回 `Result<T, String>`，错误序列化为可读字符串

前端封装在 `src/services/`，统一经 `invokeWithTimeout`（8s 超时）调用。

---

## 对话管理

```typescript
invoke("create_conversation", {
  providerId?: string, modelId?: string, title?: string,
  systemPrompt?: string, agentId?: string
}): Promise<Conversation>   // agentId 会把 agent 的 system_prompt 写入并标记 metadata.agent_id

invoke("list_conversations", {
  search?: string, limit?: number, offset?: number, agentId?: string
}): Promise<Conversation[]>  // agentId 时按 metadata LIKE 过滤该 Agent 的会话

invoke("get_conversation", { id: string }): Promise<Conversation>
invoke("update_conversation", {
  id: string, title?: string, systemPrompt?: string,
  pinned?: boolean, archived?: boolean
}): Promise<Conversation>
invoke("delete_conversation", { id: string }): Promise<void>
```

## 消息

```typescript
invoke("get_messages", { conversationId: string, limit?: number, offset?: number }): Promise<Message[]>
invoke("search_messages", { query: string, limit?: number }): Promise<SearchResult[]>
invoke("update_message", { id: string, content: string }): Promise<Message>  // 仅 user 消息可编辑
invoke("delete_messages_from", { conversationId: string, fromMessageId: string }): Promise<void>
// delete_from 会校验 fromMessageId 存在，不存在返回错误（不删光）
```

## 聊天（流式）

```typescript
invoke("send_message", {
  conversationId: string, content: string,
  apiKey: string, apiBaseUrl?: string, model?: string,
  agentMode?: boolean, images?: string[]  // dataURL
}): Promise<string>   // 返回 assistant message_id

invoke("stop_generation", { conversationId: string }): Promise<void>
// 真正取消：触发 ActiveStream.cancel.notify_waiters + cancelled 标志，
// chat_with_tools 的 select! 中断 HTTP 流，保留已生成部分文本

invoke("regenerate", {
  conversationId: string, messageId: string,
  apiKey: string, apiBaseUrl?: string, model?: string, agentMode?: boolean
}): Promise<string>  // 删 message_id 及其后消息，重新生成
```

## Agent

```typescript
invoke("list_agents"): Promise<Agent[]>
invoke("create_agent", { name, description?, systemPrompt? }): Promise<string>
invoke("update_agent", { id, updates }): Promise<void>  // system_prompt 需同时传 system_prompt 和 systemPrompt
invoke("delete_agent", { id }): Promise<void>
invoke("get_agent", { id }): Promise<Agent>
```

## 知识库（RAG）

```typescript
invoke("create_kb", { name, description }): Promise<KnowledgeBase>
invoke("list_kbs"): Promise<KnowledgeBase[]>
invoke("delete_kb", { id }): Promise<void>  // 事务删 chunks/documents/knowledge_bases
invoke("list_docs", { kbId }): Promise<Document[]>
invoke("import_document", { kbId, filePath, apiKey, apiBaseUrl?, embeddingModel? }): Promise<Document>
invoke("import_folder", { kbId, folderPath, apiKey, apiBaseUrl?, embeddingModel? }): Promise<Document[]>
// 嵌入批次失败时中止整篇文档并标 error（不 zip 错配）
invoke("search_kb", { query, kbId?, topK? }): Promise<SearchResult[]>
invoke("delete_document", { id }): Promise<void>
invoke("get_document_content", { id }): Promise<string>  // 拼接所有 chunk 全文
invoke("update_document_content", { id, content, apiKey, apiBaseUrl? }): Promise<void>  // 删旧 chunk 重新分块嵌入
invoke("rename_document", { id, newName }): Promise<Document>
invoke("batch_delete_documents", { ids: string[] }): Promise<void>
```

## 插件

```typescript
invoke("list_plugins"): Promise<PluginManifest[]>
invoke("toggle_plugin", { name, enabled }): Promise<void>
invoke("execute_plugin_tool", { toolName, args }): Promise<string>  // toolName 格式 "plugin.tool"
invoke("get_plugin_config", { name }): Promise<Record<string, unknown>>
invoke("set_plugin_config", { name, config }): Promise<void>
```

## 设置 / 统计 / 导出 / 日志 / 测试

```typescript
invoke("get_setting", { key }): Promise<string | null>
invoke("set_setting", { key, value }): Promise<void>
invoke("get_usage_stats"): Promise<UsageStats>
invoke("export_conversation", { conversationId, format }): Promise<string>
invoke("import_conversation", { data }): Promise<Conversation>
invoke("log_event", { level, message }): Promise<void>
invoke("get_log_path"): Promise<string>
invoke("get_logs", { ... }): Promise<...>
invoke("ping"): Promise<string>            // IPC 健康检查
invoke("test_chat", { ... }): Promise<...>  // API 连通测试
```

---

## 事件（后端 → 前端）

### 流式块
```typescript
listen("chat:stream-chunk", (e) => {
  // e.payload: { conversation_id, message_id, delta_text: string|null, finish_reason: string|null }
})
```

### 工具调用
```typescript
listen("chat:tool-call", (e) => {
  // e.payload: { tool_name, tool_input, tool_output, status: "success"|"error" }
})
```

### 生成完成 / 错误
```typescript
listen("chat:gen-complete", (e) => {
  // e.payload: { conversation_id, message_id, usage: { prompt_tokens, completion_tokens, total_tokens } }
})
listen("chat:gen-error", (e) => {
  // e.payload: { conversation_id, message_id, error: string }
  // 前端 handleStreamError：保留已生成部分为助手消息，清流，设置 error
})
```

### 跨窗口同步（前端 → 前端）
```typescript
// 设置窗口改动 settingsStore/kbStore 后广播，主窗口 listen 后 reload
emit("ripple:settings-changed")
```

---

## 前端调用示例

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// 流式事件在 App 顶层由 useStreamEvents hook 统一注册，转发到 chatStore：
//   chat:stream-chunk → appendToStreaming
//   chat:gen-complete → finalizeStreaming（用 payload.conversation_id 落库）
//   chat:gen-error    → handleStreamError（保留部分 + 清流 + 设错误）
//   chat:tool-call    → addToolEvent(activeId, payload)
```

`send_message` 立即返回 message_id，流式经事件推送；前端 `sendMessage` 在 await 前先置 `streamingText:""`，`appendToStreaming` 锁存首块 message_id，避免快模型首块竞态丢字。
