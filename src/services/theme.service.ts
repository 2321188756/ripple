import { invokeWithTimeout } from "./invoke";
import type { ThemeDefinition } from "@/types/theme";

export const themeService = {
  /** 获取所有主题 */
  list: (): Promise<ThemeDefinition[]> =>
    invokeWithTimeout<ThemeDefinition[]>("list_themes"),

  /** 保存所有主题 */
  saveAll: (themes: ThemeDefinition[]): Promise<void> =>
    invokeWithTimeout<void>("save_themes", { themes }),

  /** 导出主题为 JSON 文件路径 */
  exportTheme: (id: string, filePath: string): Promise<void> =>
    invokeWithTimeout<void>("export_theme", { id, filePath }),

  /** 从 JSON 文件导入主题 */
  importTheme: (filePath: string): Promise<ThemeDefinition> =>
    invokeWithTimeout<ThemeDefinition>("import_theme", { filePath }),

  /** 删除主题（内置不可删，当前使用中的由前端拦截） */
  deleteTheme: (id: string): Promise<void> =>
    invokeWithTimeout<void>("delete_theme", { id }),

  /** AI 根据关键词生成 3 套候选主题（后端调 LLM，可能耗时 10-30s） */
  generate: (keyword: string): Promise<ThemeDefinition[]> =>
    invokeWithTimeout<ThemeDefinition[]>("generate_theme", { keyword }, 90000),
};
