import { useEffect, useState, useCallback, useRef } from "react";
import {
  Plus, Trash2, FileText, FolderOpen, Loader2, Square, SquareCheckBig,
  Eye, Pencil, Info, Trash,
} from "lucide-react";
import { ContextMenu } from "@/components/common/ContextMenu";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useKBStore } from "@/stores/kbStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { kbService } from "@/services/kb.service";
import { open } from "@tauri-apps/plugin-dialog";
import { DocCard } from "./knowledge/DocCard";
import { DocPreviewDialog } from "./knowledge/DocPreviewDialog";
import { ConfirmDeleteDialog, type ConfirmDeleteState } from "./knowledge/ConfirmDeleteDialog";
import { DocPropsDialog, type DocMeta } from "./knowledge/DocPropsDialog";

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
  const [confirmDelete, setConfirmDelete] = useState<ConfirmDeleteState | null>(null);

  // 选择模式
  const [selectMode, setSelectMode] = useState<string | null>(null); // kbId
  const [selected, setSelected] = useState<Set<string>>(new Set());

  // 右键菜单
  const [ctxPos, setCtxPos] = useState<{ x: number; y: number } | null>(null);
  const [ctxDoc, setCtxDoc] = useState<DocMeta | null>(null);

  // 属性对话框
  const [showProps, setShowProps] = useState<DocMeta | null>(null);

  // 重命名
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameVal, setRenameVal] = useState("");

  const openCtxMenu = useCallback((e: React.MouseEvent, doc: DocMeta) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxPos({ x: e.clientX, y: e.clientY });
    setCtxDoc(doc);
  }, []);

  const closeCtxMenu = useCallback(() => {
    setCtxPos(null);
    setCtxDoc(null);
  }, []);

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
      return; // handleBatchDelete 已清 confirmDelete
    } else {
      await handleDeleteDoc(confirmDelete.id!, confirmDelete.kbId!);
    }
    setConfirmDelete(null);
  };

  // 打开预览
  const handleOpen = async (docId: string, docName: string, docType: string, asEdit: boolean) => {
    const reqId = ++openReqRef.current;
    setPreview({ id: docId, name: docName, type: docType });
    setEditing(asEdit);
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
                  {kbDocs.map((doc) => (
                    <DocCard
                      key={doc.id}
                      doc={doc}
                      kbId={kb.id}
                      inSelectMode={inSelectMode}
                      isSelected={selected.has(doc.id)}
                      isRenaming={renaming === doc.id}
                      renameVal={renameVal}
                      onOpen={(id, name, ext) => handleOpen(id, name, ext, false)}
                      onToggleSelect={toggleSelect}
                      onContextMenu={openCtxMenu}
                      onRenameStart={(d) => { setRenaming(d.id); setRenameVal(d.file_name); }}
                      onRenameValChange={setRenameVal}
                      onRenameCommit={handleRename}
                      onRenameCancel={() => setRenaming(null)}
                      onDelete={(id, name, kbid) => setConfirmDelete({ type: "doc", id, name, kbId: kbid })}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        );
      })}

      <ConfirmDeleteDialog
        state={confirmDelete}
        onConfirm={executeDelete}
        onCancel={() => setConfirmDelete(null)}
      />

      {/* 右键菜单 */}
      <ContextMenu
        position={ctxPos}
        onClose={closeCtxMenu}
        items={[
          {
            label: "打开编辑",
            icon: <Eye className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) handleOpen(ctxDoc.id, ctxDoc.name, ctxDoc.ext, true); },
          },
          {
            label: "打开预览",
            icon: <FileText className="w-3.5 h-3.5" />,
            onSelect: () => { if (ctxDoc) handleOpen(ctxDoc.id, ctxDoc.name, ctxDoc.ext, false); },
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

      <DocPropsDialog doc={showProps} onClose={() => setShowProps(null)} />

      <DocPreviewDialog
        preview={preview}
        content={previewContent}
        loading={previewLoading}
        editing={editing}
        saving={saving}
        onContentChange={setPreviewContent}
        onStartEdit={() => setEditing(true)}
        onCancelEdit={() => setEditing(false)}
        onSave={handleSave}
        onClose={() => { setPreview(null); setEditing(false); }}
      />
    </div>
  );
}
