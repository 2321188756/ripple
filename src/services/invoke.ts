import { invoke } from "@tauri-apps/api/core";
import { IPC_TIMEOUT } from "@/lib/constants";

/**
 * IPC 调用包装：带超时，避免后端无响应时前端永久阻塞。
 * 与原 chatStore 中的 invokeWithTimeout 行为一致。
 */
export async function invokeWithTimeout<T>(
  cmd: string,
  args?: Record<string, unknown>,
  timeoutMs: number = IPC_TIMEOUT,
): Promise<T> {
  return Promise.race([
    invoke<T>(cmd, args),
    new Promise<never>((_, reject) =>
      setTimeout(
        () => reject(new Error(`IPC timeout: ${cmd} after ${timeoutMs}ms`)),
        timeoutMs,
      ),
    ),
  ]);
}

/** 不带超时的原始 invoke 别名，用于 fire-and-forget 场景。 */
export { invoke };
