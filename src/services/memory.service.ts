import { invoke } from "./invoke";
import type { MemoryFileMeta, MemoryStats } from "@/types";

export type MemoryIndexState = "current" | "stale" | "missing";

export interface MemoryFileEntry {
  agent_id: string;
  agent_name: string;
  file_path: string;
  file_hash: string;
  indexed_hash: string | null;
  chunk_count: number;
  modified: string;
  size: number;
  index_state: MemoryIndexState;
}

export interface MemoryAgentOverview {
  agent_id: string;
  agent_name: string;
  file_count: number;
  indexed_file_count: number;
  stale_file_count: number;
  total_chunks: number;
  files: MemoryFileEntry[];
}

export interface MemoryOverview {
  agent_count: number;
  file_count: number;
  indexed_file_count: number;
  stale_file_count: number;
  total_chunks: number;
  agents: MemoryAgentOverview[];
}

export interface MemoryWriteResult {
  file_path: string;
  indexed_files: number;
}

export const memoryService = {
  reindex: (agentId: string): Promise<number> =>
    invoke<number>("reindex_memories", { agentId }),
  listFiles: (agentId: string): Promise<MemoryFileMeta[]> =>
    invoke<MemoryFileMeta[]>("list_memory_files", { agentId }),
  getFile: (agentId: string, filePath: string): Promise<string> =>
    invoke<string>("get_memory_file", { agentId, filePath }),
  deleteFile: (agentId: string, filePath: string): Promise<void> =>
    invoke<void>("delete_memory_file", { agentId, filePath }),
  stats: (agentId: string): Promise<MemoryStats> =>
    invoke<MemoryStats>("memory_stats", { agentId }),
  overview: (): Promise<MemoryOverview> =>
    invoke<MemoryOverview>("memory_overview"),
  generateTags: (agentId: string): Promise<number> =>
    invoke<number>("generate_memory_tags", { agentId }),
  saveFile: (agentId: string, filePath: string, content: string): Promise<MemoryWriteResult> =>
    invoke<MemoryWriteResult>("save_memory_file", { agentId, filePath, content }),
  openDir: (agentId: string): Promise<void> =>
    invoke<void>("open_memory_dir", { agentId }),
};
