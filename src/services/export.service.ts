import { invoke } from "./invoke";

export const exportService = {
  /** 导出对话为 markdown / json，返回内容字符串 */
  exportConversation: (
    id: string,
    format: "markdown" | "json" = "markdown",
  ): Promise<string> =>
    invoke<string>("export_conversation", { id, format }),

  /** 导入对话（JSON 字符串），返回新对话 id */
  importConversation: (jsonData: string): Promise<string> =>
    invoke<string>("import_conversation", { jsonData }),
};
