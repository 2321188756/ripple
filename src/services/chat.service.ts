import { invokeWithTimeout, invoke } from "./invoke";

export interface SendMessageParams {
  conversationId: string;
  content: string;
  apiBaseUrl: string;
  model: string;
  agentMode: boolean;
  images?: string[];
  temperature?: number;
  maxTokens?: number;
  topP?: number;
  userMessageId: string;
}

export const chatService = {
  /** 发送消息，返回后端分配的 message id */
  send: (params: SendMessageParams): Promise<string> =>
    invokeWithTimeout<string>("send_message", params as unknown as Record<string, unknown>),

  /** 停止当前对话的流式生成 */
  stop: (conversationId: string): Promise<void> =>
    invoke<void>("stop_generation", { conversationId }),

  /** 重生成：删除指定消息之后的内容，重新生成 */
  regenerate: (params: {
    conversationId: string;
    messageId: string;
    apiBaseUrl: string;
    model?: string;
    agentMode?: boolean;
    temperature?: number;
    maxTokens?: number;
    topP?: number;
  }): Promise<string> =>
    invokeWithTimeout<string>("regenerate", params as unknown as Record<string, unknown>),

  /** 更新消息内容 */
  updateMsg: (id: string, content: string): Promise<void> =>
    invoke<void>("update_message", { id, content }),

  /** 删除消息及之后所有消息 */
  deleteMsgFrom: (conversationId: string, fromMessageId: string): Promise<void> =>
    invoke<void>("delete_messages_from", { conversationId, fromMessageId }),
};
