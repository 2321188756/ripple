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
};
