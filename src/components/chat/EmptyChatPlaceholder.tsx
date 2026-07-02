import { Sparkles, ArrowUp } from "lucide-react";

/** 无对话时的空状态占位 */
export function EmptyChatPlaceholder() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-4 p-8">
      {/* Logo */}
      <div className="relative">
        <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-primary to-violet-500 flex items-center justify-center shadow-lg shadow-primary/20">
          <Sparkles className="w-8 h-8 text-white" />
        </div>
        <div className="absolute -bottom-1 -right-1 w-5 h-5 rounded-full bg-emerald-400 border-2 border-background flex items-center justify-center">
          <ArrowUp className="w-2.5 h-2.5 text-white rotate-45" />
        </div>
      </div>

      <div className="text-center space-y-1.5">
        <h2 className="text-base font-semibold text-foreground">欢迎使用 Ripple</h2>
        <p className="text-sm text-muted-foreground max-w-xs">
          选择左侧 Agent 开始对话
        </p>
      </div>

      <div className="flex gap-2 text-[11px] text-muted-foreground mt-2">
        <kbd className="px-2 py-0.5 rounded-md bg-muted border border-border font-mono">Ctrl+N</kbd>
        <span className="self-center">新建对话</span>
        <kbd className="px-2 py-0.5 rounded-md bg-muted border border-border font-mono ml-2">Ctrl+,</kbd>
        <span className="self-center">设置</span>
      </div>
    </div>
  );
}
