import { useEffect, useState } from "react";
import {
  Plus, Trash2, FileText, FileCode, Loader2, Eye, X, Save, Pencil,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter,
} from "@/components/ui/dialog";
import { useKBStore } from "@/stores/kbStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { kbService } from "@/services/kb.service";
import { open } from "@tauri-apps/plugin-dialog";

const FILE_CFG: Record<string, { icon: typeof FileText; color: string; label: string }> = {
  md:   { icon: FileText, color: "text-blue-500",  label: "MD" },
  txt:  { icon: FileText, color: "text-slate-500", label: "TXT" },
  rs:   { icon: FileCode, color: "text-amber-500", label: "Rust" },
  py:   { icon: FileCode, color: "text-emerald-500", label: "Python" },
  js:   { icon: FileCode, color: "text-yellow-500", label: "JS" },
  ts:   { icon: FileCode, color: "text-cyan-500",   label: "TS" },
  pdf:  { icon: FileText, color: "text-red-500",    label: "PDF" },
};
const DEFAULT_CFG = { icon: FileText, color: "text-muted-foreground", label: "?" };

function FileCfg(ext: string) { return FILE_CFG[ext] || DEFAULT_CFG; }

export function KnowledgePanel() {
  const { kbs, docs, loadKBs, loadDocs, createKB, deleteKB, importDoc } = useKBStore();
  const settings = useSettingsStore();
  const [newName, setNewName] = useState("");
  const [importing, setImporting] = useState<string | null>(null);

  // 预览/编辑
  const [preview, setPreview] = useState<{ id: string; name: string; type: string } | null>(null);
  const [previewContent, setPreviewContent] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);

  // 删除确认
  const [confirmDelete, setConfirmDelete] = useState<{ type: "kb" | "doc"; id: string; name: string; kbId?: string } | null>(null);

  useEffect(() => { loadKBs(); }, [loadKBs]);
  useEffect(() => {
    kbs.forEach((kb) => { if (!docs[kb.id]) loadDocs(kb.id); });
  }, [kbs, docs, loadDocs]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    await createKB(newName.trim());
    setNewName("");
  };
  const handleImport = async (kbId: string) => {
    setImporting(kbId);
    try {
      const file = await open({
        multiple: false,
        filters: [{ name: "Documents", extensions: ["txt", "md", "pdf", "rs", "py", "js", "ts"] }],
      });
      if (file) { await importDoc(kbId, file as string); await loadDocs(kbId); }
    } catch (e) { console.error(e); }
    setImporting(null);
  };
  const handleDeleteDoc = async (docId: string, kbId: string) => {
    await kbService.deleteDoc(docId);
    await loadDocs(kbId);
  };

  const executeDelete = async () => {
    if (!confirmDelete) return;
    if (confirmDelete.type === "kb") {
      await deleteKB(confirmDelete.id);
    } else {
      await handleDeleteDoc(confirmDelete.id, confirmDelete.kbId!);
    }
    setConfirmDelete(null);
  };

  // 打开预览
  const handleOpen = async (docId: string, docName: string, docType: string) => {
    setPreview({ id: docId, name: docName, type: docType });
    setEditing(false);
    setPreviewLoading(true);
    try {
      setPreviewContent(await kbService.getDocContent(docId));
    } catch (e) { setPreviewContent(`[加载失败: ${e}]`); }
    setPreviewLoading(false);
  };

  // 保存编辑
  const handleSave = async () => {
    if (!preview) return;
    setSaving(true);
    try {
      await kbService.updateDocContent({
        id: preview.id,
        content: previewContent,
        apiKey: settings.apiKey,
        apiBaseUrl: settings.apiBaseUrl,
      });
      setEditing(false);
      // 刷新 KB 中的文档列表
      const kbs = useKBStore.getState().kbs;
      for (const kb of kbs) {
        const d = (useKBStore.getState().docs[kb.id] || []).find((x) => x.id === preview.id);
        if (d) { await useKBStore.getState().loadDocs(kb.id); break; }
      }
    } catch (e) { console.error("save failed", e); }
    setSaving(false);
  };

  return (
    <div className="space-y-5">
      {/* 新建 KB */}
      <div className="flex gap-2">
        <Input value={newName} onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") handleCreate(); }}
          placeholder="新建知识库..." className="h-9 text-sm" />
        <Button onClick={handleCreate} disabled={!newName.trim()}>
          <Plus className="w-4 h-4 mr-1.5" />创建
        </Button>
      </div>

      {kbs.length === 0 && (
        <div className="text-center py-12 text-muted-foreground">
          <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
          <p className="text-sm">还没有知识库</p>
        </div>
      )}

      {kbs.map((kb) => {
        const kbDocs = docs[kb.id] || [];
        return (
          <div key={kb.id} className="border border-border rounded-xl overflow-hidden">
            {/* KB Header */}
            <div className="flex items-center justify-between px-4 py-2.5 bg-muted/30 border-b border-border">
              <div className="flex items-center gap-2">
                <span className="font-medium text-sm">{kb.name}</span>
                <span className="text-xs text-muted-foreground">{kbDocs.length} 文档</span>
              </div>
              <div className="flex gap-1">
                <Button variant="outline" size="sm" className="h-7 text-xs"
                  onClick={() => handleImport(kb.id)} disabled={importing === kb.id}>
                  {importing === kb.id ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : <Plus className="w-3 h-3 mr-1" />}
                  导入
                </Button>
                <Button variant="ghost" size="icon" className="h-7 w-7 text-muted-foreground hover:text-destructive"
                  onClick={() => setConfirmDelete({ type: "kb", id: kb.id, name: kb.name })}><Trash2 className="w-3.5 h-3.5" /></Button>
              </div>
            </div>

            {/* Docs grid — 紧凑 4 列 */}
            <div className="p-3">
              {kbDocs.length === 0 ? (
                <div className="text-center py-6 text-xs text-muted-foreground">暂无文档</div>
              ) : (
                <div className="grid grid-cols-4 gap-2">
                  {kbDocs.map((doc) => {
                    const ext = doc.file_type || "txt";
                    const cfg = FileCfg(ext);
                    const Icon = cfg.icon;
                    return (
                      <div key={doc.id}
                        className="group relative bg-muted/20 border border-border rounded-lg px-3 py-2.5 cursor-pointer
                                   hover:border-primary/40 hover:bg-accent/30 hover:shadow-sm transition-all duration-150"
                        onClick={() => handleOpen(doc.id, doc.file_name, ext)}>
                        {/* top row: icon + status */}
                        <div className="flex items-center justify-between mb-1.5">
                          <Icon className={`w-4 h-4 ${cfg.color}`} />
                          <Badge variant={doc.status === "ready" ? "success" : doc.status === "error" ? "destructive" : "warning"}
                            className="text-[9px] px-1 py-0 leading-none">{doc.status}</Badge>
                        </div>
                        {/* file name */}
                        <div className="text-xs font-medium truncate mb-1" title={doc.file_name}>{doc.file_name}</div>
                        {/* bottom: type + date */}
                        <div className="flex items-center justify-between text-[10px] text-muted-foreground">
                          <span className="bg-muted px-1 rounded">{cfg.label}</span>
                          <span>{doc.created_at?.slice(0, 10)}</span>
                        </div>
                        {/* delete btn */}
                        <button className="absolute top-1 right-1 w-5 h-5 rounded-full bg-background/80 opacity-0 group-hover:opacity-100
                                         flex items-center justify-center hover:bg-destructive/10 hover:text-destructive transition-all"
                          onClick={(e) => { e.stopPropagation(); setConfirmDelete({ type: "doc", id: doc.id, name: doc.file_name, kbId: kb.id }); }}>
                          <X className="w-2.5 h-2.5" />
                        </button>
                        {/* eye overlay */}
                        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/5 rounded-lg flex items-center justify-center transition-colors pointer-events-none">
                          <Eye className="w-4 h-4 text-foreground/0 group-hover:text-foreground/50 transition-all" />
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        );
      })}

      {/* 删除确认 Dialog */}
      <Dialog open={!!confirmDelete} onOpenChange={() => setConfirmDelete(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">确认删除</DialogTitle>
            <DialogDescription className="text-sm pt-1">
              {confirmDelete?.type === "kb"
                ? `确定要删除知识库「${confirmDelete?.name}」吗？其中所有文档和索引数据将被永久删除，无法恢复。`
                : `确定要删除文档「${confirmDelete?.name}」吗？其所有分块和嵌入数据将被永久删除。`}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2">
            <Button variant="outline" size="sm" onClick={() => setConfirmDelete(null)}>取消</Button>
            <Button variant="destructive" size="sm" onClick={executeDelete}>
              <Trash2 className="w-3.5 h-3.5 mr-1" />确认删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 预览 + 编辑 Dialog */}
      <Dialog open={!!preview} onOpenChange={() => { setPreview(null); setEditing(false); }}>
        <DialogContent className="max-w-3xl h-[85vh] flex flex-col p-0 gap-0">
          <DialogHeader className="px-5 py-3 border-b border-border flex flex-row items-center justify-between shrink-0 space-y-0">
            <div className="flex items-center gap-2">
              {preview && (() => { const Icon = FileCfg(preview.type).icon; return <Icon className="w-4 h-4 text-primary" />; })()}
              <DialogTitle className="text-sm">{preview?.name}</DialogTitle>
            </div>
            <div className="flex gap-1.5">
              {editing ? (
                <>
                  <Button size="sm" className="h-7 text-xs" onClick={handleSave} disabled={saving}>
                    <Save className="w-3 h-3 mr-1" />{saving ? "保存中..." : "保存并重新索引"}
                  </Button>
                  <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setEditing(false)}>取消</Button>
                </>
              ) : (
                <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setEditing(true)}>
                  <Pencil className="w-3 h-3 mr-1" />编辑
                </Button>
              )}
            </div>
          </DialogHeader>

          <div className="flex-1 min-h-0 overflow-hidden">
            {previewLoading ? (
              <div className="space-y-2 p-5">
                {[0, 1, 2, 3, 4, 5].map((i) => (
                  <div key={i} className="h-4 bg-muted animate-pulse rounded w-full" />
                ))}
              </div>
            ) : editing ? (
              <Textarea
                value={previewContent}
                onChange={(e) => setPreviewContent(e.target.value)}
                className="w-full h-full min-h-full rounded-none border-0 resize-none font-mono text-xs leading-relaxed p-5 focus-visible:ring-0"
                placeholder="文档内容..."
              />
            ) : (
              <ScrollArea className="h-full">
                <pre className="text-xs font-mono whitespace-pre-wrap break-all p-5 leading-relaxed select-text">
                  {previewContent}
                </pre>
              </ScrollArea>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
