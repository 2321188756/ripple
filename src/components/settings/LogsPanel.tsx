import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useLogs } from "@/hooks/useLogs";

interface LogsPanelProps {
  active: boolean;
}

/** 日志面板：自动轮询，仅底部时自动滚 */
export function LogsPanel({ active }: LogsPanelProps) {
  const { logLines, refresh, scrollRef } = useLogs(active);

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="flex justify-between mb-2">
        <span className="text-xs text-muted-foreground">{logLines.length} 行</span>
        <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={refresh}>
          <RefreshCw className="w-3 h-3 mr-1" />
          刷新
        </Button>
      </div>
      <pre
        ref={scrollRef}
        className="bg-zinc-950 text-green-300 text-[11px] p-3 rounded-lg overflow-auto max-h-[50vh] leading-relaxed font-mono"
      >
        {logLines.length === 0 ? "(empty)" : logLines.map((l, i) => <div key={i}>{l}</div>)}
      </pre>
    </div>
  );
}
