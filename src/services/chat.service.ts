import { invokeWithTimeout, invoke } from "./invoke";

export interface SendMessageParams {
  conversationId: string;
  content: string;
  apiKey: string;
  apiBaseUrl: string;
  model: string;
  agentMode: boolean;
}

export const chatService = {
  /** 发送消息，返回后端分配的 message id */
  send: (params: SendMessageParams): Promise<string> =>
    invokeWithTimeout<string>("send_message", params as unknown as Record<string, unknown>),

  /** 停止当前对话的流式生成 */
  stop: (conversationId: string): Promise<void> =>
    invoke<void>("stop_generation", { conversationId }),
};
