import { useEffect, useState, useCallback, useRef } from "react";
import {
  Plus, Trash2, FileText, FileCode, FolderOpen, Loader2, X, Save, Pencil, Check, Square, SquareCheckBig,
  Eye, Trash, Info,
} from "lucide-react";
import { ContextMenu } from "@/components/common/ContextMenu";
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
import { cn } from "@/lib/utils";

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
  // 打开预览的请求序号：快速连续打开 A/B 时丢弃过期的 A 结果，避免覆盖 B
  const openReqRef = useRef(0);

  // 删除确认
  const [confirmDelete, setConfirmDelete] = useState<{ type: "kb" | "doc" | "batch"; id?: string; name?: string; ids?: string[]; kbId?: string } | null>(null);

  // 选择模式
  const [selectMode, setSelectMode] = useState<string | null>(null); // kbId
  const [selected, setSelected] = useState<Set<string>>(new Set());

  // 右键菜单
  const [ctxPos, setCtxPos] = useState<{ x: number; y: number } | null>(null);
  const [ctxDoc, setCtxDoc] = useState<{ id: string; name: string; ext: string; kbId: string; docType: string; status: string; createdAt: string } | null>(null);

  // 属性对话框
  const [showProps, setShowProps] = useState<typeof ctxDoc>(null);

  const openCtxMenu = useCallback((e: React.MouseEvent, doc: typeof ctxDoc) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxPos({ x: e.clientX, y: e.clientY });
    setCtxDoc(doc);
  }, []);

  const closeCtxMenu = useCallback(() => {
    setCtxPos(null);
    setCtxDoc(null);
  }, []);

  // 重命名
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameVal, setRenameVal] = useState("");

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
  const handleImportFolder = async (kbId: string) => {
    setImporting(kbId);
    try {
      const dir = await open({ directory: true });
      if (dir) {
        await kbService.importFolder({
          kbId,
          folderPath: dir as string,
          apiKey: settings.apiKey,
          apiBaseUrl: settings.apiBaseUrl,
        });
        await loadDocs(kbId);
      }
    } catch (e) { console.error(e); }
    setImporting(null);
  };
  const handleDeleteDoc = async (docId: string, kbId: string) => {
    await kbService.deleteDoc(docId);
    await loadDocs(kbId);
  };
  const handleBatchDelete = async () => {
    if (!confirmDelete || !confirmDelete.ids || confirmDelete.ids.length === 0) return;
    await kbService.batchDeleteDocs(confirmDelete.ids);
    setSelectMode(null);
    setSelected(new Set());
    setConfirmDelete(null);
    if (confirmDelete.kbId) await loadDocs(confirmDelete.kbId);
  };

  const executeDelete = async () => {
    if (!confirmDelete) return;
    if (confirmDelete.type === "kb") {
      await deleteKB(confirmDelete.id!);
    } else if (confirmDelete.type === "batch") {
      await handleBatchDelete();
    } else {
      await handleDeleteDoc(confirmDelete.id!, confirmDelete.kbId!);
    }
    setConfirmDelete(null);
  };

  // 打开预览
  const handleOpen = async (docId: string, docName: string, docType: string) => {
    const reqId = ++openReqRef.current;
    setPreview({ id: docId, name: docName, type: docType });
    setEditing(false);
    setPreviewLoading(true);
    try {
      const content = await kbService.getDocContent(docId);
      // 快速连续打开 A/B 时，A 的请求可能晚于 B 完成；过期则丢弃，避免覆盖 B 的预览
      if (reqId !== openReqRef.current) return;
      setPreviewContent(content);
    } catch (e) {
      if (reqId !== openReqRef.current) return;
      setPreviewContent(`[加载失败: ${e}]`);
    }
    if (reqId === openReqRef.current) setPreviewLoading(false);
  };

  // 保存编辑
  const handleSave = async () => {
    if (!preview) return;
    setSaving(true);
    try {
      await kbService.updateDocContent({
        id: preview.id, content: previewContent,
        apiKey: settings.apiKey, apiBaseUrl: settings.apiBaseUrl,
      });
      setEditing(false);
      for (const kb of kbs) {
        const d = (docs[kb.id] || []).find((x) => x.id === preview.id);
        if (d) { await loadDocs(kb.id); break; }
      }
    } catch (e) { console.error(e); }
    setSaving(false);
  };

  // 重命名
  const handleRename = async (docId: string, kbId: string) => {
    if (!renameVal.trim()) { setRenaming(null); return; }
    // 与原文件名比较判断是否未改动。早期版本误把 renameVal 与 `renaming`(存的是 docId) 比较，
    // 永不相等，导致未改动也发 rename IPC。
    const orig = (docs[kbId] || []).find((d) => d.id === docId)?.file_name ?? "";
    if (renameVal.trim() === orig) { setRenaming(null); return; }
    try {
      await kbService.renameDoc(docId, renameVal.trim());
      await loadDocs(kbId);
    } catch (e) { console.error(e); }
    setRenaming(null);
  };

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
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
        const inSelectMode = selectMode === kb.id;
        return (
          <div key={kb.id} className="border border-border rounded-xl overflow-hidden">
            {/* KB Header */}
            <div className="flex items-center justify-between px-4 py-2.5 bg-muted/30 border-b border-border">
              <div className="flex items-center gap-2">
                <span className="font-medium text-sm">{kb.name}</span>
                <span className="text-xs text-muted-foreground">{kbDocs.length} 文档</span>
              </div>
              <div className="flex gap-1">
                {inSelectMode && selected.size > 0 && (
                  <Button variant="destructive" size="sm" className="h-7 text-xs"
                    onClick={() => setConfirmDelete({ type: "batch", ids: Array.from(selected), kbId: kb.id, name: `${selected.size} 个文档` })}>
                    <Trash2 className="w-3 h-3 mr-1" />删除 {selected.size}
                  </Button>
                )}
                <Button variant={inSelectMode ? "default" : "outline"} size="sm" className="h-7 text-xs"
                  onClick={() => { setSelectMode(inSelectMode ? null : kb.id); setSelected(new Set()); }}>
                  {inSelectMode ? <SquareCheckBig className="w-3 h-3 mr-1" /> : <Square className="w-3 h-3 mr-1" />}
                  {inSelectMode ? "完成" : "选择"}
                </Button>
                <Button variant="outline" size="sm" className="h-7 text-xs"
                  onClick={() => handleImportFolder(kb.id)} disabled={importing === kb.id}>
                  {importing === kb.id ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : <FolderOpen className="w-3 h-3 mr-1" />}
                  文件夹
                </Button>
                <Button variant="outline" size="sm" className="h-7 text-xs"
                  onClick={() => handleImport(kb.id)} disabled={importing === kb.id}>
                  <Plus className="w-3 h-3 mr-1" />文件
                </Button>
                <Button variant="ghost" size="icon" className="h-7 w-7 text-muted-foreground hover:text-destructive"
                  onClick={() => setConfirmDelete({ type: "kb", id: kb.id, name: kb.name })}>
                  <Trash2 className="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>

            {/* Docs grid */}
            <div className="p-3">
              {kbDocs.length === 0 ? (
                <div className="text-center py-6 text-xs text-muted-foreground">暂无文档</div>
              ) : (
                <div className="grid grid-cols-4 gap-2">
                  {kbDocs.map((doc) => {
                    const ext = doc.file_type || "txt";
                    const cfg = FileCfg(ext);
                    const Icon = cfg.icon;
                    const isSelected = selected.has(doc.id);
                    const isRenaming = renaming === doc.id;
                    return (
                      <div key={doc.id}
                        className={cn(
                          "group relative bg-muted/20 border rounded-lg px-3 py-2.5 transition-all duration-150",
                          inSelectMode
                            ? isSelected
                              ? "border-primary/60 bg-primary/5 cursor-pointer"
                              : "border-border hover:border-primary/30 cursor-pointer"
                            : "border-border hover:border-primary/40 hover:bg-accent/30 hover:shadow-sm cursor-pointer",
                        )}
                        onClick={() => {
                          if (inSelectMode) { toggleSelect(doc.id); return; }
                          handleOpen(doc.id, doc.file_name, ext);
                        }}
                        onContextMenu={(e) => {
                          if (inSelectMode) return;
                          openCtxMenu(e, { id: doc.id, name: doc.file_name, ext, kbId: kb.id, docType: doc.file_type, status: doc.status, createdAt: doc.created_at });
                        }}
                      >
                        {/* Select checkbox */}
                        {inSelectMode && (
                          <div className="absolute top-1 left-1 z-10">
                            <div className={cn(
                              "w-4 h-4 rounded border-2 flex items-center justify-center transition-colors",
                              isSelected ? "bg-primary border-primary" : "bg-background border-muted-foreground/40",
                            )}>
                              {isSelected && <Check className="w-3 h-3 text-primary-foreground" />}
                            </div>
                          </div>
                        )}
                        {/* top row: icon + status */}
                        <div className="flex items-center justify-between mb-1.5">
                          <Icon className={`w-4 h-4 ${cfg.color}`} />
                          <Badge variant={doc.status === "ready" ? "success" : doc.status === "error" ? "destructive" : "warning"}
                            className="text-[9px] px-1 py-0 leading-none">{doc.status}</Badge>
                        </div>
                        {/* file name (editable) */}
                        {isRenaming ? (
                          <div className="flex items-center gap-1 mb-1" onClick={(e) => e.stopPropagation()}>
                            <Input
                              autoFocus
                              value={renameVal}
                              onChange={(e) => setRenameVal(e.target.value)}
                              onBlur={() => handleRename(doc.id, kb.id)}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") handleRename(doc.id, kb.id);
                                if (e.key === "Escape") setRenaming(null);
                              }}
                              className="h-6 text-xs px-1"
                            />
                          </div>
                        ) : (
                          <div className="flex items-center gap-1 mb-1 group/name">
                            <div
                              className="text-xs font-medium truncate flex-1"
                              title={doc.file_name}
                            >{doc.file_name}</div>
                            {!inSelectMode && (
                              <Pencil
                                className="w-3 h-3 shrink-0 text-muted-foreground opacity-0 group-hover/name:opacity-100 cursor-pointer hover:text-primary transition-all"
                                onClick={(e) => { e.stopPropagation(); setRenaming(doc.id); setRenameVal(doc.file_name); }}
                              />
                            )}
                          </div>
                        )}
                        {/* bottom: type + date */}
                        <div className="flex items-center justify-between text-[10px] text-muted-foreground">
                          <span className="bg-muted px-1 rounded">{cfg.label}</span>
                          <span>{doc.created_at?.slice(0, 10)}</span>
                        </div>

                        {/* Delete button (hidden in select mode) */}
                        {!inSelectMode && (
                          <button
                            className="absolute top-1 right-1 w-5 h-5 rounded-full bg-background/80 opacity-0 group-hover:opacity-100
                                       flex items-center justify-center hover:bg-destructive/10 hover:text-destructive transition-all"
                            onClick={(e) => { e.stopPropagation(); setConfirmDelete({ type: "doc", id: doc.id, name: doc.file_name, kbId: kb.id }); }}
                          >
                            <X className="w-2.5 h-2.5" />
                          </button>
                        )}
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
                ? `确定要删除知识库「${confirmDelete?.name}」吗？所有文档和索引将被永久删除。`
                : confirmDelete?.type === "batch"
                  ? `确定要删除选中的 ${confirmDelete?.name} 吗？`
                  : `确定要删除文档「${confirmDelete?.name}」吗？`}
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

      {/* 右键菜单 */}
      <ContextMenu
        position={ctxPos}
        onClose={closeCtxMenu}
        items={[
          {
            label: "打开编辑",
            icon: <Eye className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) { setEditing(true); handleOpen(ctxDoc.id, ctxDoc.name, ctxDoc.ext); } },
          },
          {
            label: "打开预览",
            icon: <FileText className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) { setEditing(false); handleOpen(ctxDoc.id, ctxDoc.name, ctxDoc.ext); } },
          },
          {
            label: "重命名",
            icon: <Pencil className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) { setRenaming(ctxDoc.id); setRenameVal(ctxDoc.name); } },
          },
          {
            label: "属性",
            icon: <Info className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) setShowProps(ctxDoc); },
          },
          { label: "", separator: true, onSelect: () => {} },
          {
            label: "删除",
            icon: <Trash className="w-3.5 h-3.5" />,
            danger: true,
            onSelect: () => { if (ctxDoc) setConfirmDelete({ type: "doc", id: ctxDoc.id, name: ctxDoc.name, kbId: ctxDoc.kbId }); },
          },
        ]}
      />

      {/* 属性对话框 */}
      <Dialog open={!!showProps} onOpenChange={() => setShowProps(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-base">文档属性</DialogTitle>
          </DialogHeader>
          {showProps && (
            <div className="space-y-2 text-xs">
              <div className="flex justify-between py-1 border-b border-border">
                <span className="text-muted-foreground">文件名</span>
                <span className="font-medium">{showProps.name}</span>
              </div>
              <div className="flex justify-between py-1 border-b border-border">
                <span className="text-muted-foreground">类型</span>
                <span className="font-medium">{showProps.docType || "txt"}</span>
              </div>
              <div className="flex justify-between py-1 border-b border-border">
                <span className="text-muted-foreground">状态</span>
                <Badge variant={showProps.status === "ready" ? "success" : "destructive"}>{showProps.status}</Badge>
              </div>
              <div className="flex justify-between py-1 border-b border-border">
                <span className="text-muted-foreground">创建日期</span>
                <span className="font-medium">{showProps.createdAt?.slice(0, 10)}</span>
              </div>
              <div className="flex justify-between py-1 border-b border-border">
                <span className="text-muted-foreground">ID</span>
                <span className="font-mono text-[10px] text-muted-foreground truncate max-w-[180px]">{showProps.id}</span>
              </div>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setShowProps(null)}>关闭</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 预览 + 编辑 Dialog */}
      <Dialog open={!!preview} onOpenChange={() => { setPreview(null); setEditing(false); }}>
        <DialogContent className="max-w-4xl w-[85vw] h-[80vh] flex flex-col p-0 gap-0 overflow-hidden">
          <DialogHeader className="px-5 py-3 border-b border-border flex flex-row items-center justify-between shrink-0 space-y-0 pr-12">
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
                className="w-full h-full min-h-full rounded-none border-0 resize-none font-mono text-sm leading-relaxed p-6 focus-visible:ring-0"
                placeholder="文档内容..."
              />
            ) : (
              <ScrollArea className="h-full">
                <pre className="text-sm font-mono whitespace-pre-wrap break-all p-6 leading-relaxed select-text text-foreground/90">
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
