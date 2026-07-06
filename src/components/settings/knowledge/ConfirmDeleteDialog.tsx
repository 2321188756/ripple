import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter,
} from "@/components/ui/dialog";

/** 删除确认弹窗的状态。KB / 单文档 / 批量三种复用。 */
export interface ConfirmDeleteState {
  type: "kb" | "doc" | "batch";
  id?: string;
  name?: string;
  ids?: string[];
  kbId?: string;
}

interface ConfirmDeleteDialogProps {
  state: ConfirmDeleteState | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 通用删除确认弹窗（KB / 文档 / 批量复用）。从 KnowledgePanel 拆出。 */
export function ConfirmDeleteDialog({ state, onConfirm, onCancel }: ConfirmDeleteDialogProps) {
  return (
    <Dialog open={!!state} onOpenChange={onCancel}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-base">确认删除</DialogTitle>
          <DialogDescription className="text-sm pt-1">
            {state?.type === "kb"
              ? `确定要删除知识库「${state?.name}」吗？所有文档和索引将被永久删除。`
              : state?.type === "batch"
                ? `确定要删除选中的 ${state?.name} 吗？`
                : `确定要删除文档「${state?.name}」吗？`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="gap-2">
          <Button variant="outline" size="sm" onClick={onCancel}>取消</Button>
          <Button variant="destructive" size="sm" onClick={onConfirm}>
            <Trash2 className="w-3.5 h-3.5 mr-1" />确认删除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
