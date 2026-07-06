import { Save, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { FileCfg } from "./file-cfg";

interface DocPreviewDialogProps {
  preview: { id: string; name: string; type: string } | null;
  content: string;
  loading: boolean;
  editing: boolean;
  saving: boolean;
  onContentChange: (v: string) => void;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSave: () => void;
  onClose: () => void;
}

/** 文档预览/编辑对话框。从 KnowledgePanel 拆出。 */
export function DocPreviewDialog({
  preview, content, loading, editing, saving,
  onContentChange, onStartEdit, onCancelEdit, onSave, onClose,
}: DocPreviewDialogProps) {
  const Icon = preview ? FileCfg(preview.type).icon : null;
  return (
    <Dialog open={!!preview} onOpenChange={onClose}>
      <DialogContent className="max-w-4xl w-[85vw] h-[80vh] flex flex-col p-0 gap-0 overflow-hidden">
        <DialogHeader className="px-5 py-3 border-b border-border flex flex-row items-center justify-between shrink-0 space-y-0 pr-12">
          <div className="flex items-center gap-2">
            {Icon && <Icon className="w-4 h-4 text-primary" />}
            <DialogTitle className="text-sm">{preview?.name}</DialogTitle>
          </div>
          <div className="flex gap-1.5">
            {editing ? (
              <>
                <Button size="sm" className="h-7 text-xs" onClick={onSave} disabled={saving}>
                  <Save className="w-3 h-3 mr-1" />{saving ? "保存中..." : "保存并重新索引"}
                </Button>
                <Button size="sm" variant="outline" className="h-7 text-xs" onClick={onCancelEdit}>取消</Button>
              </>
            ) : (
              <Button size="sm" variant="outline" className="h-7 text-xs" onClick={onStartEdit}>
                <Pencil className="w-3 h-3 mr-1" />编辑
              </Button>
            )}
          </div>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-hidden">
          {loading ? (
            <div className="space-y-2 p-5">
              {[0, 1, 2, 3, 4, 5].map((i) => (
                <div key={i} className="h-4 bg-muted animate-pulse rounded w-full" />
              ))}
            </div>
          ) : editing ? (
            <Textarea
              value={content}
              onChange={(e) => onContentChange(e.target.value)}
              className="w-full h-full min-h-full rounded-none border-0 resize-none font-mono text-sm leading-relaxed p-6 focus-visible:ring-0"
              placeholder="文档内容..."
            />
          ) : (
            <ScrollArea className="h-full">
              <pre className="text-sm font-mono whitespace-pre-wrap break-all p-6 leading-relaxed select-text text-foreground/90">
                {content}
              </pre>
            </ScrollArea>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
