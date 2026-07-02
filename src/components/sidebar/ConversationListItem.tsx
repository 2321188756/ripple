import { useState } from "react";
import { Pin, PinOff, Trash2, Pencil, Check, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import type { Conversation } from "@/types";

interface ConversationListItemProps {
  conv: Conversation;
  active: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onRename: (title: string) => void;
  onTogglePin: () => void;
}

/** 侧边栏单个对话项：选中/重命名/置顶/删除 */
export function ConversationListItem({
  conv,
  active,
  onSelect,
  onDelete,
  onRename,
  onTogglePin,
}: ConversationListItemProps) {
  const [editing, setEditing] = useState(false);
  const [val, setVal] = useState(conv.title);

  const commit = () => {
    if (val.trim() && val !== conv.title) onRename(val.trim());
    setEditing(false);
  };

  return (
    <div
      className={cn(
        "group flex items-center px-3 py-2.5 border-b border-border text-sm cursor-pointer transition-colors hover:bg-accent/60",
        active && "bg-primary/10 border-l-2 border-l-primary",
      )}
    >
      {editing ? (
        <div className="flex-1 flex items-center gap-1">
          <Input
            autoFocus
            value={val}
            onChange={(e) => setVal(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
            className="h-6 text-xs px-1 py-0.5"
            onClick={(e) => e.stopPropagation()}
          />
          <Button variant="ghost" size="icon" className="h-5 w-5" onClick={commit}>
            <Check className="w-3 h-3" />
          </Button>
          <Button variant="ghost" size="icon" className="h-5 w-5" onClick={() => setEditing(false)}>
            <X className="w-3 h-3" />
          </Button>
        </div>
      ) : (
        <>
          <div
            className="flex-1 min-w-0"
            onClick={onSelect}
            onDoubleClick={() => {
              setVal(conv.title);
              setEditing(true);
            }}
          >
            <div className="truncate font-medium text-xs flex items-center gap-1">
              {conv.pinned && <Pin className="w-2.5 h-2.5 text-amber-500 inline" />}
              {conv.title}
            </div>
            <div className="text-[10px] text-muted-foreground mt-0.5">
              {new Date(conv.updated_at).toLocaleDateString()}
            </div>
          </div>
          <div className="opacity-0 group-hover:opacity-100 flex items-center gap-0.5 ml-1">
            {active && (
              <>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6"
                      onClick={(e) => {
                        e.stopPropagation();
                        onTogglePin();
                      }}
                      aria-label={conv.pinned ? "取消置顶" : "置顶"}
                    >
                      {conv.pinned ? (
                        <PinOff className="w-3 h-3 text-amber-500" />
                      ) : (
                        <Pin className="w-3 h-3" />
                      )}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{conv.pinned ? "取消置顶" : "置顶"}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6"
                      onClick={(e) => {
                        e.stopPropagation();
                        setVal(conv.title);
                        setEditing(true);
                      }}
                      aria-label="重命名"
                    >
                      <Pencil className="w-3 h-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>重命名</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6 hover:text-destructive"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete();
                      }}
                      aria-label="删除"
                    >
                      <Trash2 className="w-3 h-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>删除</TooltipContent>
                </Tooltip>
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}
