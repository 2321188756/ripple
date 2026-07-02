import { invokeWithTimeout } from "./invoke";
import type { Conversation } from "@/types";

export const conversationService = {
  list: (params: {
    agentId?: string;
    limit?: number;
    offset?: number;
  } = {}): Promise<Conversation[]> =>
    invokeWithTimeout<Conversation[]>("list_conversations", {
      limit: 100,
      offset: 0,
      ...params,
    }),

  create: (params: {
    agentId?: string;
    title?: string;
    systemPrompt?: string;
  } = {}): Promise<Conversation> =>
    invokeWithTimeout<Conversation>("create_conversation", params),

  get: (id: string): Promise<Conversation> =>
    invokeWithTimeout<Conversation>("get_conversation", { id }),

  update: (
    id: string,
    updates: {
      title?: string;
      pinned?: boolean;
      archived?: boolean;
      modelId?: string;
    },
  ): Promise<Conversation> =>
    invokeWithTimeout<Conversation>("update_conversation", { id, ...updates }),

  delete: (id: string): Promise<void> =>
    invokeWithTimeout<void>("delete_conversation", { id }),
};
