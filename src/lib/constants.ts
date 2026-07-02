/**
 * 应用全局常量
 */

/** 可用模型列表（与 App.tsx 原有 select option 保持一致） */
export const MODELS = [
  { value: "deepseek-v4-flash", label: "deepseek-v4-flash" },
  { value: "deepseek-v4-pro", label: "deepseek-v4-pro" },
  { value: "[薄荷]gemini-3-flash-preview", label: "gemini-3-flash" },
  { value: "[薄荷]gemini-3-pro-high", label: "gemini-3-pro-high" },
  { value: "[黑与白]gemini-2.5-pro", label: "gemini-2.5-pro" },
] as const;

/** 全局快捷键映射 */
export const KEYBOARD_SHORTCUTS = {
  newChat: { key: "n", ctrl: true, description: "新建对话" },
  search: { key: "k", ctrl: true, description: "搜索消息" },
  settings: { key: "l", ctrl: true, description: "打开设置" },
  escape: { key: "Escape", ctrl: false, description: "关闭面板" },
} as const;

/** 自动滚动阈值：距底部多少 px 内算"靠近底部" */
export const AUTO_SCROLL_THRESHOLD = 100;

/** 日志轮询间隔（ms） */
export const LOG_POLL_INTERVAL = 3000;

/** IPC 默认超时（ms） */
export const IPC_TIMEOUT = 8000;

/** 设置面板 localStorage 键名 */
export const SETTINGS_PANEL_KEYS = {
  x: "ripple_settings_x",
  y: "ripple_settings_y",
  w: "ripple_settings_w",
  h: "ripple_settings_h",
} as const;

/** 上下文压缩默认配置 */
export const CONTEXT_DEFAULTS: {
  enabled: boolean;
  recentWindow: string;
  summaryInterval: string;
  maxTokens: string;
} = {
  enabled: true,
  recentWindow: "20",
  summaryInterval: "10",
  maxTokens: "32000",
};
