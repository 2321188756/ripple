import { invoke } from "./invoke";
import type { MemoryFileMeta, MemoryStats } from "@/types";

export interface MemoryFileEntry {
  agent_name: string;
  file_path: string;
  file_hash: string;
  modified: string;
  size: number;
}

export const memoryService = {
  /** 重建指定 Agent 的记忆索引，返回重建的文件数 */
  reindex: (agentId: string): Promise<number> =>
    invoke<number>("reindex_memories", { agentId }),

  /** 列出 Agent 的记忆文件 */
  listFiles: (agentId: string): Promise<MemoryFileMeta[]> =>
    invoke<MemoryFileMeta[]>("list_memory_files", { agentId }),

  /** 读取记忆文件内容 */
  getFile: (filePath: string): Promise<string> =>
    invoke<string>("get_memory_file", { filePath }),

  /** 删除文件（同时清理索引） */
  deleteFile: (agentId: string, filePath: string): Promise<void> =>
    invoke<void>("delete_memory_file", { agentId, filePath }),

  /** 记忆统计 */
  stats: (agentId: string): Promise<MemoryStats> =>
    invoke<MemoryStats>("memory_stats", { agentId }),

  /** 列出所有 Agent 的记忆文件 */
  listAllFiles: (): Promise<MemoryFileEntry[]> =>
    invoke<MemoryFileEntry[]>("list_all_memory_files"),

  /** 递归扫描 dailynote/ 下所有 .txt/.md，为没有 tag 行的文件生成并追加 tag，返回处理的文件数 */
  generateTags: (): Promise<number> =>
    invoke<number>("generate_memory_tags"),

  /** 保存记忆文件内容（编辑后写回） */
  saveFile: (filePath: string, content: string): Promise<void> =>
    invoke<void>("save_memory_file", { filePath, content }),

  /** 删除指定 Agent 的记忆文件（同时清理索引） */
  deleteAgentFile: (agentName: string, filePath: string): Promise<void> =>
    invoke<void>("delete_agent_memory_file", { agentName, filePath }),

  /** 在系统文件管理器中打开 Agent 记忆目录 */
  openDir: (agentName: string): Promise<void> =>
    invoke<void>("open_memory_dir", { agentName }),
};
