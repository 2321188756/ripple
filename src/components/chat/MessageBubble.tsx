import { memo, useCallback, useMemo } from "react";
import { User, Sparkles, Expand } from "lucide-react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
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
    <div className={cn("flex gap-2.5 px-4 py-1.5", isUser ? "justify-end" : "justify-start")}>
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
            ? "max-w-[80%] bg-primary text-primary-foreground shadow-md shadow-primary/20 rounded-br-md"
            : "max-w-[98%] bg-card border border-border text-card-foreground shadow-sm rounded-bl-md",
          "rounded-2xl px-4 py-2.5 text-sm leading-relaxed animate-fade-in",
        )}
      >
        {/* 图片渲染 */}
        {hasImages && (
          <div className={cn("flex flex-wrap gap-1.5", text ? "mb-2" : "")}>
            {images.map((url, i) => (
              <div key={i} className="group relative w-24 h-24 rounded-lg overflow-hidden border border-border/50 bg-muted/50">
                <img
                  src={url}
                  alt={`image-${i}`}
                  className="w-full h-full object-cover cursor-pointer"
                  onClick={() => openPreview(url)}
                />
                <div className="absolute bottom-0.5 right-0.5 w-4 h-4 rounded bg-black/40 text-white flex items-center justify-center
                                opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                     onClick={() => openPreview(url)}>
                  <Expand className="w-2.5 h-2.5" />
                </div>
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
