import { Sparkles } from "lucide-react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";

interface StreamingMessageProps {
  text: string;
}

/** 流式输出中的助手消息占位，带闪烁光标 */
export function StreamingMessage({ text }: StreamingMessageProps) {
  return (
    <div
      role="status"
      aria-live="off"
      aria-label="助手正在回复"
      className="flex gap-2.5 px-4 py-1.5 justify-start"
    >
      <Avatar className="h-6 w-6 mt-0.5 shrink-0">
        <AvatarFallback className="bg-primary/15 text-primary">
          <Sparkles className="w-3 h-3" />
        </AvatarFallback>
      </Avatar>
      <div className="max-w-[80%] rounded-xl px-4 py-2.5 text-sm leading-relaxed bg-card border border-border border-l-2 border-l-primary text-card-foreground">
        <span className="whitespace-pre-wrap">{text}</span>
        <span className="inline-block w-2 h-4 bg-primary ml-0.5 animate-pulse rounded-sm align-middle" />
      </div>
    </div>
  );
}
