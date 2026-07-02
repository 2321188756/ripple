import { invokeWithTimeout } from "./invoke";
import type { Message, SearchResult } from "@/types";

export const messageService = {
  list: (
    conversationId: string,
    limit: number = 1000,
  ): Promise<Message[]> =>
    invokeWithTimeout<Message[]>("get_messages", { conversationId, limit }),

  search: (query: string, limit: number = 50): Promise<SearchResult[]> =>
    invokeWithTimeout<SearchResult[]>("search_messages", { query, limit }),
};
