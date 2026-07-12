import { memo, useCallback, useMemo } from "react";
import { User, Sparkles, Expand } from "lucide-react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import MarkdownRenderer from "@/components/MarkdownRenderer";
import { cn } from "@/lib/utils";
import type { ContentBlock } from "@/types";

interface MessageBubbleProps {
  role: string;
  content: ContentBlock[];
}

/** 单条消息气泡（memo 缓存，旧消息不重渲染）。
 *  接收原始 content blocks，内部 useMemo 提取 text/images，
 *  避免父组件每次渲染 .filter().map() 产生新数组引用击穿 memo。 */
export const MessageBubble = memo(function MessageBubble({ role, content }: MessageBubbleProps) {
  const isUser = role === "user";

  const { text, images } = useMemo(() => {
    let t = "";
    const imgs: string[] = [];
    for (const b of content) {
      if (b.type === "text") t += b.text;
      else if (b.type === "image" && b.url) imgs.push(b.url);
    }
    return { text: t, images: imgs };
  }, [content]);
  const hasImages = images.length > 0;

  const openPreview = useCallback((url: string) => {
    window.dispatchEvent(new CustomEvent("ripple:preview-image", { detail: { url } }));
  }, []);

  return (
    <div className={cn("flex gap-2.5 px-3 py-2 sm:px-5", isUser ? "justify-end" : "justify-start")}>
      {!isUser && (
        <Avatar className="h-7 w-7 mt-0.5 shrink-0 ring-1 ring-border/50">
          <AvatarFallback className="bg-gradient-to-br from-primary/15 to-primary-300/15 text-primary">
            <Sparkles className="w-3.5 h-3.5" />
          </AvatarFallback>
        </Avatar>
      )}
      <div
        className={cn(
          isUser
            ? "max-w-[82%] rounded-br-md bg-primary text-primary-foreground shadow-primary"
            : "max-w-[98%] rounded-bl-md border border-border bg-card text-card-foreground shadow-xs",
          "rounded-2xl px-4 py-3 text-sm leading-relaxed",
        )}
      >
        {/* 图片渲染 */}
        {hasImages && (
          <div className={cn("flex flex-wrap gap-1.5", text ? "mb-2" : "")}>
            {images.map((url, i) => (
              <div key={i} className="group relative h-24 w-24 overflow-hidden rounded-lg border border-border/50 bg-muted/50">
                <Button
                  type="button"
                  variant="ghost"
                  className="h-full w-full rounded-none p-0 focus-visible:ring-offset-0"
                  onClick={() => openPreview(url)}
                  aria-label={`预览图片 ${i + 1}`}
                >
                  <img src={url} alt={`消息图片 ${i + 1}`} className="h-full w-full object-cover" />
                </Button>
                <span className="pointer-events-none absolute bottom-1 right-1 flex h-5 w-5 items-center justify-center rounded bg-black/45 text-white opacity-0 transition-opacity group-hover:opacity-100">
                  <Expand className="h-3 w-3" />
                </span>
              </div>
            ))}
          </div>
        )}
        {text && <MarkdownRenderer content={text} />}
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
