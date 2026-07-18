import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Brain, FileCode, FileText, FolderOpen, Pencil, RefreshCw, Tags, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useMemoryLab } from "@/hooks/useMemoryLab";
import { memoryService, type MemoryFileEntry } from "@/services/memory.service";

export const MemoryLabPanel = memo(function MemoryLabPanel() {
  const { overview, loading, error, refresh } = useMemoryLab();
  const agents = overview?.agents ?? [];
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [preview, setPreview] = useState<MemoryFileEntry | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<MemoryFileEntry | null>(null);
  const [content, setContent] = useState("");
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const previewRequestRef = useRef(0);

  useEffect(() => {
    if (!selectedAgentId && agents[0]) setSelectedAgentId(agents[0].agent_id);
    if (selectedAgentId && !agents.some((agent) => agent.agent_id === selectedAgentId)) {
      setSelectedAgentId(agents[0]?.agent_id ?? null);
    }
  }, [agents, selectedAgentId]);

  const selectedAgent = useMemo(
    () => agents.find((agent) => agent.agent_id === selectedAgentId) ?? null,
    [agents, selectedAgentId],
  );

  const openFile = useCallback(async (file: MemoryFileEntry) => {
    const requestId = ++previewRequestRef.current;
    setPreview(file);
    setEditing(false);
    setBusy(true);
    setActionError(null);
    try {
      const nextContent = await memoryService.getFile(file.agent_id, file.file_path);
      if (requestId === previewRequestRef.current) setContent(nextContent);
    } catch (cause) {
      if (requestId === previewRequestRef.current) setActionError(String(cause));
    } finally {
      if (requestId === previewRequestRef.current) setBusy(false);
    }
  }, []);

  const save = useCallback(async () => {
    if (!preview) return;
    setBusy(true);
    setActionError(null);
    try {
      await memoryService.saveFile(preview.agent_id, preview.file_path, content);
      await refresh();
      setEditing(false);
    } catch (cause) { setActionError(String(cause)); }
    finally { setBusy(false); }
  }, [content, preview, refresh]);

  const remove = useCallback(async () => {
    if (!deleteTarget) return;
    setBusy(true);
    setActionError(null);
    try {
      await memoryService.deleteFile(deleteTarget.agent_id, deleteTarget.file_path);
      if (preview?.file_path === deleteTarget.file_path) setPreview(null);
      setDeleteTarget(null);
      await refresh();
    } catch (cause) { setActionError(String(cause)); }
    finally { setBusy(false); }
  }, [deleteTarget, preview, refresh]);

  const reindex = useCallback(async () => {
    if (!selectedAgent) return;
    setBusy(true);
    setActionError(null);
    try { await memoryService.reindex(selectedAgent.agent_id); await refresh(); }
    catch (cause) { setActionError(String(cause)); }
    finally { setBusy(false); }
  }, [refresh, selectedAgent]);

  if (loading && !overview) return <div className="p-5 text-xs text-muted-foreground">加载中...</div>;

  return <div className="flex h-full">
    <aside className="w-48 shrink-0 border-r border-border flex flex-col">
      <div className="px-3 py-2.5 border-b border-border flex items-center justify-between">
        <span className="text-xs font-medium">Agent</span>
        <Button variant="ghost" size="sm" className="h-6 w-6 p-0" onClick={() => void refresh()} aria-label="刷新记忆"><RefreshCw className="w-3 h-3" /></Button>
      </div>
      <ScrollArea className="flex-1 p-1.5">
        {agents.map((agent) => <button key={agent.agent_id} onClick={() => setSelectedAgentId(agent.agent_id)} className={cn("w-full flex items-center gap-2 px-2.5 py-2 rounded-md text-xs text-left", selectedAgentId === agent.agent_id ? "bg-primary/10 text-primary font-medium" : "text-muted-foreground hover:bg-accent hover:text-foreground")}>
          <Brain className="w-3.5 h-3.5" /><span className="truncate flex-1">{agent.agent_name}</span><span className="text-[10px]">{agent.file_count}</span>
        </button>)}
      </ScrollArea>
    </aside>

    <section className="flex-1 min-w-0 flex flex-col">
      {selectedAgent && <header className="px-4 py-2.5 border-b border-border flex items-center justify-between gap-3">
        <div><div className="text-xs font-medium">{selectedAgent.agent_name}</div><div className="text-[10px] text-muted-foreground">{selectedAgent.indexed_file_count}/{selectedAgent.file_count} 已索引 · {selectedAgent.total_chunks} 个分块{selectedAgent.stale_file_count > 0 ? ` · ${selectedAgent.stale_file_count} 待同步` : ""}</div></div>
        <div className="flex gap-1">
          <Button variant="ghost" size="sm" className="h-7 text-[11px]" disabled={busy} onClick={() => void reindex()}><RefreshCw className="w-3 h-3 mr-1" />重建索引</Button>
          <Button variant="ghost" size="sm" className="h-7 text-[11px]" disabled={busy} onClick={async () => { setActionError(null); try { await memoryService.generateTags(selectedAgent.agent_id); await reindex(); } catch (cause) { setActionError(String(cause)); } }}><Tags className="w-3 h-3 mr-1" />批量标签</Button>
          <Button variant="ghost" size="sm" className="h-7 text-[11px]" onClick={async () => { try { await memoryService.openDir(selectedAgent.agent_id); } catch (cause) { setActionError(String(cause)); } }}><FolderOpen className="w-3 h-3 mr-1" />打开目录</Button>
        </div>
      </header>}
      {(error || actionError) && <div role="alert" className="mx-4 mt-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{error || actionError}</div>}
      <ScrollArea className="flex-1 p-4">
        {!selectedAgent?.files.length ? <div className="py-12 text-center text-muted-foreground"><FileText className="w-8 h-8 mx-auto mb-2 opacity-30" /><p className="text-sm">暂无记忆文件</p></div> : <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-3">
          {selectedAgent.files.map((file) => <button key={file.file_path} onClick={() => void openFile(file)} className="rounded-lg border border-border bg-card p-3 text-left hover:bg-accent transition-colors">
            <div className="flex items-start gap-2"><FileText className="w-4 h-4 text-primary shrink-0" /><span className="text-xs font-medium truncate">{file.file_path.split("/").pop()}</span></div>
            <div className="mt-2 text-[10px] text-muted-foreground">{(file.size / 1024).toFixed(1)} KB · {file.chunk_count} 分块</div>
            <div className={cn("mt-1 text-[10px]", file.index_state === "current" ? "text-primary" : "text-destructive")}>{file.index_state === "current" ? "索引已同步" : file.index_state === "stale" ? "内容已变更" : "尚未索引"}</div>
          </button>)}
        </div>}
      </ScrollArea>
    </section>

    <Dialog open={Boolean(preview)} onOpenChange={(open) => { if (!open) setPreview(null); }}><DialogContent className="max-w-3xl max-h-[85vh] flex flex-col"><DialogHeader><DialogTitle className="text-sm flex items-center gap-2"><FileCode className="w-4 h-4" />{preview?.file_path}</DialogTitle></DialogHeader>{actionError && <div role="alert" className="text-xs text-destructive">{actionError}</div>}<div className="flex-1 min-h-0">{editing ? <Textarea value={content} onChange={(event) => setContent(event.target.value)} className="min-h-[320px] font-mono text-xs" /> : <ScrollArea className="max-h-[50vh]"><pre className="p-2 text-xs whitespace-pre-wrap font-mono">{busy ? "加载中..." : content}</pre></ScrollArea>}</div><DialogFooter>{editing ? <><Button variant="outline" size="sm" onClick={() => setEditing(false)}>取消</Button><Button size="sm" disabled={busy} onClick={() => void save()}>{busy ? "保存中..." : "保存并同步索引"}</Button></> : <><Button variant="outline" size="sm" onClick={() => setEditing(true)}><Pencil className="w-3 h-3 mr-1" />编辑</Button><Button variant="destructive" size="sm" onClick={() => preview && setDeleteTarget(preview)}><Trash2 className="w-3 h-3 mr-1" />删除</Button></>}</DialogFooter></DialogContent></Dialog>

    <Dialog open={Boolean(deleteTarget)} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}><DialogContent className="max-w-md"><DialogHeader><DialogTitle>删除记忆文件？</DialogTitle></DialogHeader><p className="text-sm text-muted-foreground">将永久删除「{deleteTarget?.file_path}」及其索引。此操作无法撤销。</p><DialogFooter><Button variant="outline" onClick={() => setDeleteTarget(null)}>取消</Button><Button variant="destructive" disabled={busy} onClick={() => void remove()}>{busy ? "删除中..." : "确认删除"}</Button></DialogFooter></DialogContent></Dialog>
  </div>;
});
