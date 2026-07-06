import { useState } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { AlertTriangle } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";

/**
 * 工具调用审批弹窗。
 * requires_approval 的插件工具（shell-exec / code-runner / file-ops 等）执行前，
 * 后端 emit chat:tool-approval-request 并阻塞等待；此弹窗让用户批准/拒绝后才继续。
 *
 * 按 Agent 权限级别决定交互：
 *  - strict：每次问，不显示「信任此工具」
 *  - elevated：问，可勾「信任此工具」→ 记录后该工具后续自动放行
 *  - full：后端不 emit，不弹此框
 */
export function ApprovalDialog() {
  const current = useChatStore((s) => s.pendingApprovals[0]);
  const resolveApproval = useChatStore((s) => s.resolveApproval);
  const [trustTool, setTrustTool] = useState(false);

  if (!current) return null;

  const allowTrust = current.permission_level === "elevated";

  let argsText: string;
  try {
    argsText = JSON.stringify(current.arguments, null, 2);
  } catch {
    argsText = String(current.arguments);
  }

  const close = (approved: boolean) => {
    resolveApproval(current.request_id, approved, allowTrust && trustTool);
    setTrustTool(false);
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) close(false); }}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-sm">
            <AlertTriangle className="w-4 h-4 text-amber-500" />
            工具调用审批
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 text-xs">
          <p className="text-muted-foreground">
            AI 请求执行工具 <span className="font-mono font-medium text-foreground">{current.tool_name}</span>，请确认是否允许。
          </p>
          <div>
            <div className="text-muted-foreground mb-1">参数：</div>
            <ScrollArea className="max-h-48 rounded border border-border bg-muted/30">
              <pre className="p-2 font-mono text-[11px] whitespace-pre-wrap break-all">{argsText}</pre>
            </ScrollArea>
          </div>
          <p className="text-amber-600 dark:text-amber-400 text-[11px] flex items-start gap-1">
            <AlertTriangle className="w-3 h-3 mt-0.5 shrink-0" />
            <span>此工具可能执行命令、代码或文件操作，批准后将立即运行。如不确认参数安全性，请拒绝。</span>
          </p>
          {allowTrust && (
            <label className="flex items-center gap-2 cursor-pointer select-none text-xs text-foreground">
              <input
                type="checkbox"
                checked={trustTool}
                onChange={(e) => setTrustTool(e.target.checked)}
                className="cursor-pointer"
              />
              信任此工具（该 Agent 后续调用此工具不再询问）
            </label>
          )}
        </div>
        <DialogFooter className="gap-2">
          <Button variant="outline" size="sm" onClick={() => close(false)}>
            拒绝
          </Button>
          <Button size="sm" onClick={() => close(true)}>
            批准执行
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
