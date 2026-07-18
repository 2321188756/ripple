import { invokeWithTimeout } from "./invoke";
import type { ThemeDefinition } from "@/types/theme";
import { validateThemeContrast, type ThemeValidationWarning } from "@/lib/theme-contrast";

export type PreparedTheme = {
  theme: ThemeDefinition;
  warnings: ThemeValidationWarning[];
};

/** Validates and corrects a theme before the UI persists it. */
export function prepareThemeForSave(theme: ThemeDefinition): PreparedTheme {
  return validateThemeContrast(theme);
}

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

  /** AI 根据需求描述生成 3 套候选主题（prompt 为完整模板内容；image 为 base64 图片供 vision 模型取色） */
  generate: (prompt: string, model?: string, image?: string): Promise<ThemeDefinition[]> =>
    invokeWithTimeout<ThemeDefinition[]>("generate_theme", { prompt, modelOverride: model, image }, 90000),

  /** 保存壁纸文件到 ~/ripple_wallpapers/，返回绝对路径 */
  saveWallpaper: (srcPath: string, themeId: string): Promise<string> =>
    invokeWithTimeout<string>("save_wallpaper", { srcPath, themeId }),

  /** 读取壁纸文件并返回 data:image/...;base64,... URL */
  readWallpaperBase64: (path: string): Promise<string> =>
    invokeWithTimeout<string>("read_wallpaper_base64", { path }),
};
