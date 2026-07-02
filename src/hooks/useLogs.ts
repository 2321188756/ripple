import { useCallback, useEffect, useRef, useState } from "react";
import { logService } from "@/services/log.service";
import { LOG_POLL_INTERVAL } from "@/lib/constants";

/**
 * 日志轮询 hook：定时拉取后端日志，保留滚动位置（仅底部时自动滚）。
 * @param active 是否启用轮询
 * @param pollInterval 轮询间隔 ms，默认 3000
 */
export function useLogs(active: boolean, pollInterval: number = LOG_POLL_INTERVAL) {
  const [logLines, setLogLines] = useState<string[]>([]);
  const scrollRef = useRef<HTMLPreElement>(null);

  const refresh = useCallback(async () => {
    const el = scrollRef.current;
    const wasAtBottom = el
      ? el.scrollHeight - el.scrollTop - el.clientHeight < 50
      : true;
    try {
      const lines = await logService.getLogs(200);
      setLogLines(lines);
    } catch (e) {
      setLogLines([`[error: ${e}]`]);
    }
    requestAnimationFrame(() => {
      if (wasAtBottom && el) el.scrollTop = el.scrollHeight;
    });
  }, []);

  useEffect(() => {
    if (!active) return;
    refresh();
    const iv = setInterval(refresh, pollInterval);
    return () => clearInterval(iv);
  }, [active, refresh, pollInterval]);

  return { logLines, refresh, scrollRef };
}
