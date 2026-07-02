# IPC 协议（Tauri 命令与事件）

前端通过 `invoke()` 调用后端命令，通过 `listen()` 接收后端事件。命名约定：

- **命令**：`<domain>_<action>`，snake_case（如 `conversation_create`）
- **事件**：`<domain>:<event>`，kebab-case（如 `chat:stream-chunk`）

所有命令返回 `Result<T, String>`，错误序列化为可读字符串。

---

## 对话管理

```typescript
// 创建对话
invoke("conversation_create", {
  title?: string,
  modelId?: string,
  providerId?: string,
  systemPrompt?: string
}): Promise<Conversation>

// 列表（支持搜索、分页）
invoke("conversation_list", {
  search?: string,
  limit?: number,    // 默认 50
  offset?: number
}): Promise<Conversation[]>

// 获取单个
invoke("conversation_get", { id: string }): Promise<Conversation>

// 更新（标题/系统提示/置顶/归档）
invoke("conversation_update", {
  id: string,
  title?: string,
  systemPrompt?: string,
  pinned?: boolean,
  archived?: boolean
}): Promise<Conversation>

// 删除
invoke("conversation_delete", { id: string }): Promise<void>

// 获取消息（cursor 分页）
invoke("message_list", {
  conversationId: string,
  limit?: number,     // 默认 50
  beforeId?: string   // 向前翻页游标
}): Promise<Message[]>
```

## 聊天

```typescript
// 发送消息（触发流式）
invoke("send_message", {
  conversationId: string,
  content: string,
  attachments?: Attachment[],
  knowledgeRefs?: string[]   // @引用的知识库 ID
}): Promise<string>   // 返回 assistant message_id

// 停止生成
invoke("stop_generation", { conversationId: string }): Promise<void>

// 重新生成
invoke("regenerate_message", {
  conversationId: string,
  messageId: string
}): Promise<string>

// 审批工具调用
invoke("approve_tool_call", {
  toolCallId: string,
  approved: boolean
}): Promise<void>
```

## 模型管理

```typescript
invoke("provider_list"): Promise<ProviderConfig[]>
invoke("provider_add", { config: ProviderConfig, apiKey: string }): Promise<ProviderConfig>
invoke("provider_delete", { id: string }): Promise<void>
invoke("provider_test_connection", { providerId: string }): Promise<boolean>
invoke("provider_fetch_models", { providerId: string }): Promise<ModelInfo[]>
invoke("model_set_active", { providerId: string, modelId: string }): Promise<void>
```

## 工具

```typescript
invoke("tool_list"): Promise<ToolDefinition[]>
invoke("tool_toggle", { name: string, enabled: boolean }): Promise<void>
```

## 插件

```typescript
invoke("plugin_list"): Promise<PluginManifest[]>
invoke("plugin_install", { wasmBytes: number[], manifest: PluginManifest }): Promise<PluginManifest>
invoke("plugin_uninstall", { pluginId: string }): Promise<void>
invoke("plugin_toggle", { pluginId: string, enabled: boolean }): Promise<void>
invoke("plugin_configure", { pluginId: string, config: Record<string, unknown> }): Promise<void>
```

## 知识库（RAG）

```typescript
invoke("kb_list"): Promise<KnowledgeBase[]>
invoke("kb_create", { name: string, config: KbConfig }): Promise<KnowledgeBase>
invoke("kb_delete", { id: string }): Promise<void>
invoke("kb_import_document", { kbId: string, filePath: string }): Promise<Document>
invoke("kb_search", { query: string, kbId?: string, topK?: number }): Promise<Chunk[]>
invoke("kb_reindex", { kbId: string }): Promise<void>
```

## 设置

```typescript
invoke("settings_get", { keys: string[] }): Promise<Record<string, unknown>>
invoke("settings_set", { settings: Record<string, unknown> }): Promise<void>
```

## 搜索

```typescript
invoke("search_conversations", { query: string, limit?: number }): Promise<SearchResult[]>
invoke("search_messages", { query: string, conversationId?: string, limit?: number }): Promise<SearchResult[]>
```

---

## 事件（后端 → 前端）

### 流式块
```typescript
listen("chat:stream-chunk", (e) => {
  // e.payload: { conversationId, messageId, deltaText?, deltaThinking?, toolCalls?, finishReason? }
})
```

### 工具调用
```typescript
listen("chat:tool-call", (e) => {
  // e.payload: { conversationId, messageId, toolCallId, toolName, toolInput, requiresApproval }
})

listen("chat:tool-result", (e) => {
  // e.payload: { conversationId, messageId, toolCallId, status: "success"|"error", output }
})
```

### 生成完成/错误
```typescript
listen("chat:gen-complete", (e) => {
  // e.payload: { conversationId, messageId, usage: { promptTokens, completionTokens, totalTokens } }
})

listen("chat:gen-error", (e) => {
  // e.payload: { conversationId, messageId, error, errorCode }
})
```

### 对话标题自动生成
```typescript
listen("conversation:title-generated", (e) => {
  // e.payload: { conversationId, title }
})
```

### RAG 索引进度
```typescript
listen("kb:index-progress", (e) => {
  // e.payload: { kbId, documentId, status, processed, total }
})
```

---

## 前端调用示例

```typescript
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

async function sendMessage(conversationId: string, content: string) {
  // 发送前注册事件监听
  const unlistenChunk = await listen<{ messageId: string; deltaText?: string }>(
    "chat:stream-chunk",
    (e) => {
      if (e.payload.conversationId === conversationId) {
        appendStreamingText(e.payload.messageId, e.payload.deltaText)
      }
    }
  )

  const unlistenComplete = await listen("chat:gen-complete", () => {
    unlistenChunk()
    unlistenComplete()
  })

  await invoke("send_message", { conversationId, content })
}
```
