import { useState } from "react";
import { ChevronDown, Check, AlertCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ToolCallEvent } from "@/types";

/** 工具调用卡片：可折叠，显示工具名/状态/输入/输出 */
export default function ToolCallCard({ event }: { event: ToolCallEvent }) {
  const [collapsed, setCollapsed] = useState(false);
  const isSuccess = event.status === "success";

  return (
    <div className="my-2 border border-border rounded-lg overflow-hidden text-xs bg-card">
      {/* Header */}
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2 bg-muted/50 cursor-pointer select-none hover:bg-muted transition-colors"
        onClick={() => setCollapsed(!collapsed)}
        aria-expanded={!collapsed}
      >
        <ChevronDown
          className={cn(
            "w-3 h-3 text-muted-foreground transition-transform",
            collapsed && "-rotate-90",
          )}
        />
        <span className="font-mono font-medium text-foreground/80">{event.tool_name}</span>
        <Badge
          variant={isSuccess ? "success" : "destructive"}
          className="ml-auto"
        >
          {isSuccess ? (
            <Check className="w-2.5 h-2.5 mr-0.5" />
          ) : (
            <AlertCircle className="w-2.5 h-2.5 mr-0.5" />
          )}
          {event.status}
        </Badge>
      </button>

      {/* Body */}
      {!collapsed && (
        <div className="px-3 py-2 border-t border-border bg-background space-y-1.5">
          <div>
            <span className="text-muted-foreground">Input: </span>
            <code className="text-foreground/80 break-all">{event.tool_input}</code>
          </div>
          <div>
            <span className="text-muted-foreground">Output: </span>
            <code
              className={cn(
                "break-all",
                isSuccess ? "text-emerald-600 dark:text-emerald-400" : "text-destructive",
              )}
            >
              {event.tool_output}
            </code>
          </div>
        </div>
      )}
    </div>
  );
}
