import { invokeWithTimeout, invoke } from "./invoke";

export const systemService = {
  /** IPC 健康检测 */
  ping: () => invokeWithTimeout<string>("ping", { message: "hello" }),
  /** 测试 API Key 是否可用 */
  testChat: (apiKey: string) => invoke<string>("test_chat", { apiKey }),
};
