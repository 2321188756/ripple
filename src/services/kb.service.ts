import { invoke } from "./invoke";
import type { KnowledgeBase, Document, SearchResult } from "@/types";

export interface FolderImportFailure {
  filePath: string;
  stage: string;
  error: string;
}

export interface FolderImportResult {
  imported: Document[];
  failures: FolderImportFailure[];
}

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
    apiBaseUrl: string;
    embeddingModel?: string;
  }): Promise<Document> =>
    invoke<Document>("import_document", {
      embeddingModel: "Qwen/Qwen3-Embedding-8B",
      ...params,
    }),

  /** 递归导入文件夹中所有支持的文件 */
  importFolder: (params: {
    kbId: string;
    folderPath: string;
    apiBaseUrl: string;
    embeddingModel?: string;
  }): Promise<FolderImportResult> =>
    invoke<FolderImportResult>("import_folder", {
      embeddingModel: "Qwen/Qwen3-Embedding-8B",
      ...params,
    }),

  deleteDoc: (id: string): Promise<void> =>
    invoke<void>("delete_document", { id }),

  /** 批量删除文档 */
  batchDeleteDocs: (ids: string[]): Promise<void> =>
    invoke<void>("batch_delete_documents", { ids }),

  /** 重命名文档 */
  renameDoc: (id: string, newName: string): Promise<Document> =>
    invoke<Document>("rename_document", { id, newName }),

  getDocContent: (id: string): Promise<string> =>
    invoke<string>("get_document_content", { id }),

  updateDocContent: (params: {
    id: string;
    content: string;
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
    apiBaseUrl?: string;
  }): Promise<SearchResult[]> =>
    invoke<SearchResult[]>("search_kb", params),
};
