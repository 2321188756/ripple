import { invokeWithTimeout } from "./invoke";
import type { Message, SearchResult } from "@/types";

export const messageService = {
  list: (
    conversationId: string,
    options: { limit?: number; beforeId?: string } = {},
  ): Promise<Message[]> =>
    invokeWithTimeout<Message[]>("get_messages", {
      conversationId,
      limit: options.limit ?? 50,
      beforeId: options.beforeId,
    }),

  search: (query: string, limit: number = 50): Promise<SearchResult[]> =>
    invokeWithTimeout<SearchResult[]>("search_messages", { query, limit }),
};
