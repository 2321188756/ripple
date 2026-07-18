import { Sparkles, ArrowUp } from "lucide-react";
import { useAgentStore } from "@/stores/agentStore";

/** 无对话时的空状态占位。显示当前选中 Agent（若有），让用户知道为哪个 Agent 建对话。 */
export function EmptyChatPlaceholder() {
  const selectedAgent = useAgentStore((s) => s.selectedAgent);

  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-4 p-8">
      {/* Logo / Agent 图标 */}
      <div className="relative">
        <div
          className="w-16 h-16 rounded-2xl flex items-center justify-center shadow-lg shadow-primary/20 text-3xl overflow-hidden"
          style={{ backgroundImage: "linear-gradient(to bottom right, hsl(var(--gradient-from)), hsl(var(--gradient-via)), hsl(var(--gradient-to)))" }}
        >
          {selectedAgent?.icon ? (
            selectedAgent.icon.startsWith("data:image") ? (
              <img src={selectedAgent.icon} alt="" className="w-full h-full object-cover" />
            ) : (
              <span>{selectedAgent.icon}</span>
            )
          ) : (
            <Sparkles className="w-8 h-8 text-white" />
          )}
        </div>
        <div className="absolute -bottom-1 -right-1 w-5 h-5 rounded-full bg-success border-2 border-background flex items-center justify-center">
          <ArrowUp className="w-2.5 h-2.5 text-white rotate-45" />
        </div>
      </div>

      <div className="text-center space-y-1.5">
        <h2 className="text-base font-semibold text-foreground">
          {selectedAgent ? `${selectedAgent.name}` : "欢迎使用 Ripple"}
        </h2>
        <p className="text-sm text-muted-foreground max-w-xs">
          {selectedAgent
            ? selectedAgent.description || `正在与 ${selectedAgent.name} 对话，发送消息开始`
            : "选择左侧 Agent 开始对话"}
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
