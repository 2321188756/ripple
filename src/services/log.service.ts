import { invoke } from "./invoke";

export const logService = {
  /** 写日志到后端日志文件 */
  log: (level: "info" | "warn" | "error", message: string): Promise<void> =>
    invoke<void>("log_event", { level, message }),

  /** 读取最近 N 行日志 */
  getLogs: (lines: number = 200): Promise<string[]> =>
    invoke<string[]>("get_logs", { lines }),

  /** 获取日志文件路径 */
  getLogPath: (): Promise<string> =>
    invoke<string>("get_log_path"),
};
