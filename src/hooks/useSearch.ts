import { useCallback, useRef, useState } from "react";
import { messageService } from "@/services/message.service";
import type { SearchResult } from "@/types";

/**
 * 消息搜索 hook。
 */
export function useSearch() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [showResults, setShowResults] = useState(false);
  const reqIdRef = useRef(0);

  const execute = useCallback(async () => {
    if (!query.trim()) {
      setShowResults(false);
      return;
    }
    // 请求序号：快速连续搜索时旧请求若晚到则丢弃，避免覆盖新结果
    const reqId = ++reqIdRef.current;
    try {
      const r = await messageService.search(query, 50);
      if (reqId !== reqIdRef.current) return;
      setResults(r);
      setShowResults(true);
    } catch (e) {
      console.error("search", e);
    }
  }, [query]);

  const clear = useCallback(() => {
    reqIdRef.current++;
    setQuery("");
    setResults([]);
    setShowResults(false);
  }, []);

  return { query, setQuery, results, showResults, execute, clear };
}
