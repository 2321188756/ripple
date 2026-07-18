/** 主题模式 */
export type Theme = "light" | "dark" | "system";

/** 设置面板 tab */
export type SettingsTab = "settings" | "logs" | "knowledge" | "memory" | "stats" | "plugins";

/** 侧边栏 tab */
export type SidebarTab = "agents" | "chats" | "settings";

/** 自定义主题 */
export interface ThemeDefinition {
  id: string;
  name: string;
  description?: string;
  isBuiltin?: boolean;
  colors: ThemeColors;
  agentStyle?: AgentThemeStyle;
  /** 背景壁纸文件绝对路径（可选，前端用 convertFileSrc 转 URL） */
  wallpaper?: string;
}

export interface ThemeColors {
  light: Record<string, string>;
  dark: Record<string, string>;
}

export interface AgentThemeStyle {
  iconColor?: string;
  borderColor?: string;
  borderWidth?: number;
  nameColor?: string;
}
