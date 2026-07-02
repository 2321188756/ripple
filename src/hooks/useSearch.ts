import { useCallback, useState } from "react";
import { messageService } from "@/services/message.service";
import type { SearchResult } from "@/types";

/**
 * 消息搜索 hook。
 */
export function useSearch() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [showResults, setShowResults] = useState(false);

  const execute = useCallback(async () => {
    if (!query.trim()) {
      setShowResults(false);
      return;
    }
    try {
      const r = await messageService.search(query, 50);
      setResults(r);
      setShowResults(true);
    } catch (e) {
      console.error("search", e);
    }
  }, [query]);

  const clear = useCallback(() => {
    setQuery("");
    setResults([]);
    setShowResults(false);
  }, []);

  return { query, setQuery, results, showResults, execute, clear };
}
