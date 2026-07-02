import { memo } from "react";
import { User, Sparkles } from "lucide-react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import MarkdownRenderer from "@/components/MarkdownRenderer";
import { cn } from "@/lib/utils";

interface MessageBubbleProps {
  role: string;
  text: string;
}

/** 单条消息气泡（memo 缓存，旧消息不重渲染） */
export const MessageBubble = memo(function MessageBubble({ role, text }: MessageBubbleProps) {
  const isUser = role === "user";
  return (
    <div className={cn("flex gap-2.5 px-4 py-1.5", isUser ? "justify-end" : "justify-start")}>
      {!isUser && (
        <Avatar className="h-7 w-7 mt-0.5 shrink-0 ring-1 ring-border/50">
          <AvatarFallback className="bg-gradient-to-br from-primary/15 to-violet-500/15 text-primary">
            <Sparkles className="w-3.5 h-3.5" />
          </AvatarFallback>
        </Avatar>
      )}
      <div
        className={cn(
          "max-w-[80%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed animate-fade-in",
          isUser
            ? "bg-primary text-primary-foreground shadow-md shadow-primary/20 rounded-br-md"
            : "bg-card border border-border text-card-foreground shadow-sm rounded-bl-md",
        )}
      >
        <MarkdownRenderer content={text} />
      </div>
      {isUser && (
        <Avatar className="h-7 w-7 mt-0.5 shrink-0 ring-1 ring-border/50">
          <AvatarFallback className="bg-muted">
            <User className="w-3.5 h-3.5" />
          </AvatarFallback>
        </Avatar>
      )}
    </div>
  );
});
