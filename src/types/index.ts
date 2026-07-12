// 与后端 ripple-core 类型对齐

export interface Conversation {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  model_id: string;
  provider_id: string;
  system_prompt: string | null;
  pinned: boolean;
  archived: boolean;
  metadata: Record<string, unknown>;
}

export interface Message {
  id: string;
  conversation_id: string;
  role: "system" | "user" | "assistant" | "tool";
  content: ContentBlock[];
  created_at: string;
  token_count: number | null;
  metadata: Record<string, unknown>;
}

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; url: string; detail?: string }
  | { type: "tool_call"; id: string; name: string; arguments: unknown }
  | { type: "tool_result"; tool_call_id: string; content: string }
  | { type: "thinking"; text: string };

// IPC 事件载荷
export interface StreamChunkPayload {
  conversation_id: string;
  message_id: string;
  delta_text: string | null;
  finish_reason: string | null;
}

export interface GenCompletePayload {
  conversation_id: string;
  message_id: string;
  usage: { prompt_tokens: number; completion_tokens: number; total_tokens: number };
}

export interface GenErrorPayload {
  conversation_id: string;
  message_id: string;
  error: string;
}

// 工具调用事件（chat:tool-call 载荷，对齐后端 commands/chat.rs 的 payload）
export interface ToolCallEvent {
  tool_name: string;
  tool_input: string;
  tool_output: string;
  status: "success" | "error" | "rejected";
  /** 触发此工具调用的 assistant 消息 ID */
  message_id: string;
}

// 工具审批请求（chat:tool-approval-request 载荷）
// requires_approval 的工具执行前，后端 emit 此事件，前端弹审批框，用户决定后调 approve_tool_call 回传。
export interface ApprovalRequestEvent {
  request_id: string;
  conversation_id: string;
  tool_name: string;
  arguments: unknown;
  /** Agent 当前权限级别：strict(每次问) / elevated(可信任积累) / full(全放行)。决定是否显示「信任此工具」复选框。 */
  permission_level: string;
}

export interface SearchResult {
  conversation_id: string;
  role: string;
  snippet: string;
  created_at: string;
  match_text: string;
}

// Agent
export interface Agent {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  tools: string;
  model: string;
  icon: string;
  created_at: string;
  updated_at: string;
  // 样式
  icon_color: string;
  border_color: string;
  border_width: number;
  name_color: string;
  // 模型参数
  temperature: number;
  max_tokens: number;
  top_p: number;
}

// 知识库
export interface KnowledgeBase {
  id: string;
  name: string;
  description: string;
  chunk_size: number;
  chunk_overlap: number;
  created_at: string;
}

export interface Document {
  id: string;
  kb_id: string;
  file_name: string;
  file_type: string;
  status: string;
  created_at: string;
}

// 记忆系统
export interface MemoryFileMeta {
  file_path: string;
  file_hash: string;
  chunk_count: number;
  updated_at: string;
}

export interface MemoryStats {
  file_count: number;
  total_chunks: number;
  files: MemoryFileMeta[];
}

// 用量统计
export interface UsageStats {
  total_conversations: number;
  total_messages: number;
  total_tokens: number;
  daily_stats: { date: string; messages: number; tokens: number }[];
  messages_by_role: { role: string; count: number }[];
  top_models: { model: string; conversations: number }[];
}

// 插件
export interface PluginTool {
  name: string;
  description: string;
}

export interface PluginManifest {
  name: string;
  version: string;
  mode?: "tool" | "transform" | "daemon";
  runtime?: "rhai" | "node" | "python" | "shell";
  description?: string;
  tools?: PluginTool[];
  config_schema?: {
    properties: Record<string, { description?: string; type?: string }>;
  };
}
