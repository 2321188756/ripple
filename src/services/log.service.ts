import { invoke } from "./invoke";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error" | "unknown";

export interface LogEntry {
  timestamp: string | null;
  level: LogLevel;
  target: string | null;
  message: string;
  raw: string;
}

export interface LogSnapshot {
  path: string;
  fileSize: number;
  modifiedAtMs: number | null;
  byteCap: number;
  requestedLines: number;
  returnedLines: number;
  truncated: boolean;
  entries: LogEntry[];
}

export type ClientLogEvent =
  | { event: "chat_send_ignored" }
  | { event: "chat_conversation_creating" }
  | { event: "chat_conversation_create_failed" }
  | { event: "chat_send_started"; conversationId: string; contentChars: number }
  | { event: "chat_send_succeeded"; messageId: string }
  | { event: "chat_send_failed" };

export const logService = {
  /** 写入 allowlist 中的安全前端诊断事件。 */
  log: (event: ClientLogEvent): Promise<void> =>
    invoke<void>("log_event", { event }),

  /** 读取最新日志文件的有界结构化快照 */
  getLogs: (lines: number = 500): Promise<LogSnapshot> =>
    invoke<LogSnapshot>("get_logs", { lines }),

  /** 获取日志文件路径 */
  getLogPath: (): Promise<string> =>
    invoke<string>("get_log_path"),
};
