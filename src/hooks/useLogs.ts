import { useCallback, useEffect, useRef, useState } from "react";
import { logService, type LogSnapshot } from "@/services/log.service";
import { LOG_POLL_INTERVAL } from "@/lib/constants";

const EMPTY_SNAPSHOT: LogSnapshot = {
  path: "",
  fileSize: 0,
  modifiedAtMs: null,
  byteCap: 0,
  requestedLines: 0,
  returnedLines: 0,
  truncated: false,
  entries: [],
};

/**
 * 日志轮询 hook：串行调度请求，用序号拒绝失效结果，避免重叠轮询和旧响应覆盖。
 * 自动跟随是显式状态；用户离开底部时自动关闭。
 */
export function useLogs(active: boolean, pollInterval: number = LOG_POLL_INTERVAL) {
  const [snapshot, setSnapshot] = useState<LogSnapshot>(EMPTY_SNAPSHOT);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [follow, setFollow] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const mountedRef = useRef(true);
  const activeRef = useRef(active);
  const inFlightRef = useRef(false);
  const requestIdRef = useRef(0);
  const followRef = useRef(follow);

  useEffect(() => {
    activeRef.current = active;
  }, [active]);

  useEffect(() => {
    followRef.current = follow;
  }, [follow]);

  useEffect(() => {
    // React StrictMode intentionally runs setup → cleanup → setup in development.
    // Restore the flag on every setup so the second, real mount accepts snapshots.
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      inFlightRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  const scrollToBottom = useCallback(() => {
    requestAnimationFrame(() => {
      const element = scrollRef.current;
      if (element) element.scrollTop = element.scrollHeight;
    });
  }, []);

  const refresh = useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    const requestId = ++requestIdRef.current;
    setLoading(true);
    try {
      const nextSnapshot = await logService.getLogs(500);
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      setSnapshot(nextSnapshot);
      setError(null);
      if (followRef.current) scrollToBottom();
    } catch (cause) {
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      inFlightRef.current = false;
      if (mountedRef.current && requestId === requestIdRef.current) setLoading(false);
    }
  }, [scrollToBottom]);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    const poll = async () => {
      await refresh();
      if (!cancelled && activeRef.current) timeoutId = setTimeout(poll, pollInterval);
    };
    void poll();

    return () => {
      cancelled = true;
      if (timeoutId) clearTimeout(timeoutId);
      requestIdRef.current += 1;
    };
  }, [active, pollInterval, refresh]);

  const handleScroll = useCallback(() => {
    const element = scrollRef.current;
    if (!element || !followRef.current) return;
    const atBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 50;
    if (!atBottom) setFollow(false);
  }, []);

  const setFollowing = useCallback((enabled: boolean) => {
    setFollow(enabled);
    followRef.current = enabled;
    if (enabled) scrollToBottom();
  }, [scrollToBottom]);

  return {
    snapshot,
    error,
    loading,
    follow,
    setFollowing,
    refresh,
    scrollRef,
    handleScroll,
  };
}
