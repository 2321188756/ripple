import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from "@/components/ui/dialog";

/** 文档元信息（右键菜单 / 属性弹窗用） */
export interface DocMeta {
  id: string;
  name: string;
  ext: string;
  kbId: string;
  docType: string;
  status: string;
  createdAt: string;
}

interface DocPropsDialogProps {
  doc: DocMeta | null;
  onClose: () => void;
}

/** 文档属性弹窗。从 KnowledgePanel 拆出。 */
export function DocPropsDialog({ doc, onClose }: DocPropsDialogProps) {
  return (
    <Dialog open={!!doc} onOpenChange={onClose}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="text-base">文档属性</DialogTitle>
        </DialogHeader>
        {doc && (
          <div className="space-y-2 text-xs">
            <div className="flex justify-between py-1 border-b border-border">
              <span className="text-muted-foreground">文件名</span>
              <span className="font-medium">{doc.name}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-border">
              <span className="text-muted-foreground">类型</span>
              <span className="font-medium">{doc.docType || "txt"}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-border">
              <span className="text-muted-foreground">状态</span>
              <Badge variant={doc.status === "ready" ? "success" : "destructive"}>{doc.status}</Badge>
            </div>
            <div className="flex justify-between py-1 border-b border-border">
              <span className="text-muted-foreground">创建日期</span>
              <span className="font-medium">{doc.createdAt?.slice(0, 10)}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-border">
              <span className="text-muted-foreground">ID</span>
              <span className="font-mono text-[10px] text-muted-foreground truncate max-w-[180px]">{doc.id}</span>
            </div>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose}>关闭</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
