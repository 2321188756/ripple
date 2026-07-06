import { invoke } from "./invoke";

export const settingsService = {
  get: (key: string): Promise<string | null> =>
    invoke<string | null>("get_setting", { key }),
  set: (key: string, value: string): Promise<void> =>
    invoke<void>("set_setting", { key, value }),
  setDebugLogging: (enabled: boolean): Promise<void> =>
    invoke<void>("set_debug_logging", { enabled }),
  getDebugLogging: (): Promise<boolean> =>
    invoke<boolean>("get_debug_logging"),
};

/** 便捷：返回字符串（null 转空串） */
export async function getSetting(key: string): Promise<string> {
  const v = await settingsService.get(key);
  return v ?? "";
}

export async function setSetting(key: string, value: string): Promise<void> {
  await settingsService.set(key, value);
}
