# ripple-conversation-store

SQLite 持久化层。对话/消息 CRUD、FTS5 全文搜索、设置 KV 存储。

## 已实现

- `init_db`：连接池 + WAL + 迁移
- Schema：conversations / messages / messages_fts / provider_configs / api_keys / plugins / tool_audit_log / settings
- `ConversationRepo`：创建/获取/列表(搜索)/更新/删除
- `MessageRepo`：插入/列表(cursor 分页)/FTS5 搜索
- 级联删除（删除对话时自动删除消息）

## 测试（7 个）

- create_and_get_conversation / list_conversations_with_search / update_and_delete_conversation
- insert_and_list_messages / message_pagination_by_cursor / search_messages_via_fts
- delete_conversation_cascades_messages
