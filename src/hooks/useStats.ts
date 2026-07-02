import { useEffect, useState } from "react";
import { statsService } from "@/services/stats.service";
import type { UsageStats } from "@/types";

/**
 * 用量统计 hook：挂载时拉取一次统计数据。
 */
export function useStats() {
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
    setLoading(true);
    try {
      setStats(await statsService.getUsage());
    } catch {
      setStats(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  return { stats, loading, refresh };
}
