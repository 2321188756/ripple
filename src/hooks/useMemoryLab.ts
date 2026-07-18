import { useCallback, useEffect, useState } from "react";
import { memoryService, type MemoryOverview } from "@/services/memory.service";

export function useMemoryLab() {
  const [overview, setOverview] = useState<MemoryOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setOverview(await memoryService.overview());
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  return { overview, loading, error, refresh };
}
