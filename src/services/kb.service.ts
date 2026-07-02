import { invoke } from "./invoke";
import type { KnowledgeBase, Document, SearchResult } from "@/types";

export const kbService = {
  listKBs: (): Promise<KnowledgeBase[]> =>
    invoke<KnowledgeBase[]>("list_kbs"),

  createKB: (name: string, description: string = ""): Promise<KnowledgeBase> =>
    invoke<KnowledgeBase>("create_kb", { name, description }),

  deleteKB: (id: string): Promise<void> =>
    invoke<void>("delete_kb", { id }),

  listDocs: (kbId: string): Promise<Document[]> =>
    invoke<Document[]>("list_docs", { kbId }),

  importDoc: (params: {
    kbId: string;
    filePath: string;
    apiKey: string;
    apiBaseUrl: string;
    embeddingModel?: string;
  }): Promise<Document> =>
    invoke<Document>("import_document", {
      embeddingModel: "Qwen/Qwen3-Embedding-8B",
      ...params,
    }),

  deleteDoc: (id: string): Promise<void> =>
    invoke<void>("delete_document", { id }),

  getDocContent: (id: string): Promise<string> =>
    invoke<string>("get_document_content", { id }),

  updateDocContent: (params: {
    id: string;
    content: string;
    apiKey: string;
    apiBaseUrl: string;
    embeddingModel?: string;
  }): Promise<void> =>
    invoke<void>("update_document_content", {
      embeddingModel: "Qwen/Qwen3-Embedding-8B",
      ...params,
    }),

  search: (params: {
    query: string;
    kbId?: string;
    topK?: number;
    apiKey: string;
    apiBaseUrl?: string;
  }): Promise<SearchResult[]> =>
    invoke<SearchResult[]>("search_kb", params),
};
