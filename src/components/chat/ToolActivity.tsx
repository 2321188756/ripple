import { Wrench } from "lucide-react";
import { cn } from "@/lib/utils";
import type { ContentBlock } from "@/types";

interface ToolActivityProps {
  blocks: ContentBlock[];
}

export function ToolActivity({ blocks }: ToolActivityProps) {
  const calls = blocks.filter((block) => block.type === "tool_call");
  const results = new Map(
    blocks
      .filter((block) => block.type === "tool_result")
      .map((block) => [block.tool_call_id, block.content]),
  );

  if (calls.length === 0) return null;

  return (
    <div className="mt-2 space-y-2" aria-label="工具调用">
      {calls.map((call) => {
        const result = results.get(call.id);
        return (
          <section key={call.id} className="rounded-lg border border-border bg-muted/40 p-3">
            <div className="mb-2 flex items-center gap-2 text-xs font-medium text-muted-foreground">
              <Wrench className="h-3.5 w-3.5" aria-hidden="true" />
              <span>{call.name}</span>
              <span className={cn("ml-auto", result === undefined ? "text-amber-600" : "text-emerald-600")}>
                {result === undefined ? "执行中" : "已完成"}
              </span>
            </div>
            {result !== undefined && (
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded bg-background/70 p-2 text-xs">
                <code>{result}</code>
              </pre>
            )}
          </section>
        );
      })}
    </div>
  );
}
