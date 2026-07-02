import { X } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ErrorBannerProps {
  error: string | null;
  onDismiss: () => void;
}

/** 可关闭的红色错误横幅 */
export function ErrorBanner({ error, onDismiss }: ErrorBannerProps) {
  if (!error) return null;
  return (
    <div
      role="alert"
      aria-live="polite"
      className="mx-4 mb-2 px-3 py-2 bg-destructive/10 border border-destructive/30 rounded-lg text-sm text-destructive flex justify-between items-center animate-fade-in"
    >
      <span className="break-all">{error}</span>
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6 shrink-0"
        onClick={onDismiss}
        aria-label="关闭错误"
      >
        <X className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
