import { X, RotateCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ErrorBannerProps {
  error: string | null;
  onDismiss: () => void;
  /** 提供则显示"重试"按钮（点击重试最近一次失败的 send/regenerate） */
  onRetry?: () => void;
}

/** 可关闭的红色错误横幅，可选重试 */
export function ErrorBanner({ error, onDismiss, onRetry }: ErrorBannerProps) {
  if (!error) return null;
  return (
    <div
      role="alert"
      aria-live="polite"
      className="mx-4 mb-2 px-3 py-2 bg-destructive/10 border border-destructive/30 rounded-lg text-sm text-destructive flex justify-between items-center gap-2 animate-fade-in"
    >
      <span className="break-all">{error}</span>
      <div className="flex items-center gap-1 shrink-0">
        {onRetry && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-xs"
            onClick={onRetry}
            aria-label="重试"
          >
            <RotateCw className="w-3 h-3 mr-1" />重试
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6"
          onClick={onDismiss}
          aria-label="关闭错误"
        >
          <X className="w-3.5 h-3.5" />
        </Button>
      </div>
    </div>
  );
}
