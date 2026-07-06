import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { FileText, FileCode, RefreshCw, Trash2, FolderOpen, Pencil, Brain, Tags } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { memoryService } from "@/services/memory.service";
import type { MemoryFileEntry } from "@/services/memory.service";

/** 统记忆管理（设置 → 记忆 Lab）—— 左右布局：左侧 Agent 列表，右侧卡片网格 */
export const MemoryLabPanel = memo(function MemoryLabPanel() {
  const [files, setFiles] = useState<MemoryFileEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [preview, setPreview] = useState<MemoryFileEntry | null>(null);
  const [previewContent, setPreviewContent] = useState("");
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await memoryService.listAllFiles();
      setFiles(list);
    } catch (e) { console.error(e); }
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  // 按 Agent 分组
  const agents = useMemo(() => {
    const map = new Map<string, MemoryFileEntry[]>();
    for (const f of files) {
      const list = map.get(f.agent_name) || [];
      list.push(f);
      map.set(f.agent_name, list);
    }
    return Array.from(map.entries())
      .map(([name, agentFiles]) => ({ name, files: agentFiles }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [files]);

  const currentFiles = useMemo(
    () => agents.find((a) => a.name === selectedAgent)?.files ?? [],
    [agents, selectedAgent],
  );

  // 默认选中第一个
  useEffect(() => {
    if (!selectedAgent && agents.length > 0) setSelectedAgent(agents[0].name);
  }, [agents, selectedAgent]);

  const handleOpen = useCallback(async (entry: MemoryFileEntry) => {
    setPreview(entry);
    setPreviewLoading(true);
    setEditing(false);
    try {
      const content = await memoryService.getFile(entry.file_path);
      setPreviewContent(content);
    } catch (e) { setPreviewContent(`[加载失败: ${e}]`); }
    setPreviewLoading(false);
  }, []);

  const handleSave = useCallback(async () => {
    if (!preview) return;
    setSaving(true);
    try {
      await memoryService.saveFile(preview.file_path, previewContent);
      await load();
      setEditing(false);
    } catch (e) { console.error(e); }
    setSaving(false);
  }, [preview, previewContent, load]);

  const handleDelete = useCallback(async (entry: MemoryFileEntry) => {
    if (!confirm(`确认删除「${entry.file_path}」？`)) return;
    try {
      await memoryService.deleteAgentFile(entry.agent_name, entry.file_path);
      setFiles((prev) => prev.filter((f) => f.file_path !== entry.file_path));
      if (preview?.file_path === entry.file_path) setPreview(null);
    } catch (e) { console.error(e); }
  }, [preview]);

  const handleOpenDir = useCallback((agentName: string) => {
    memoryService.openDir(agentName).catch(() => {});
  }, []);

  if (loading) return <div className="p-5 text-xs text-muted-foreground">加载中...</div>;

  const sidebar = (
    <div className="w-44 shrink-0 border-r border-border flex flex-col">
      <div className="flex items-center justify-between px-3 py-2.5 border-b border-border">
        <span className="text-xs font-medium text-foreground">Agent</span>
        <Button variant="ghost" size="sm" className="h-6 w-6 p-0" onClick={load} title="刷新">
          <RefreshCw className="w-3 h-3" />
        </Button>
      </div>
      <ScrollArea className="flex-1">
        {agents.length === 0 ? (
          <div className="p-4 text-center text-[11px] text-muted-foreground">暂无记忆</div>
        ) : (
          <div className="p-1.5 space-y-0.5">
            {agents.map(({ name, files }) => (
              <button key={name}
                onClick={() => setSelectedAgent(name)}
                className={cn(
                  "w-full flex items-center gap-2 px-2.5 py-2 rounded-md text-xs text-left transition-colors",
                  selectedAgent === name
                    ? "bg-primary/10 text-primary font-medium"
                    : "hover:bg-accent text-muted-foreground hover:text-foreground",
                )}
              >
                <Brain className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate flex-1">{name}</span>
                <span className="text-[10px] text-muted-foreground">{files.length}</span>
              </button>
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  );

  const content = (
    <div className="flex-1 flex flex-col min-w-0">
      {/* Agent 标题栏 */}
      {selectedAgent && (
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border">
          <div className="flex items-center gap-2">
            <span className="text-xs font-medium text-foreground">{selectedAgent}</span>
            <span className="text-[10px] text-muted-foreground">{currentFiles.length} 个文件</span>
          </div>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" className="h-6 text-[11px] px-2"
              onClick={() => memoryService.generateTags().then(load).catch(() => {})}>
              <Tags className="w-3 h-3 mr-1" />批量标签
            </Button>
            <Button variant="ghost" size="sm" className="h-6 text-[11px] px-2"
              onClick={() => handleOpenDir(selectedAgent)}>
              <FolderOpen className="w-3 h-3 mr-1" />打开目录
            </Button>
          </div>
        </div>
      )}

      {/* 书架 */}
      <ScrollArea className="flex-1 p-4">
        {currentFiles.length === 0 ? (
          <div className="text-center py-12 text-muted-foreground">
            <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
            <p className="text-sm">暂无记忆文件</p>
            <p className="text-xs mt-1">在对话中让 AI 调用 remember 工具</p>
          </div>
        ) : (
          <div className="flex flex-wrap gap-3 pb-1 border-b-2 border-muted-foreground/15 relative
            after:content-[''] after:absolute after:bottom-0 after:left-0 after:right-0 after:h-1 after:bg-gradient-to-b after:from-black/5 after:to-transparent">
            {currentFiles.map((f) => {
              // 根据文件名 hash 生成书脊颜色
              const hue = (f.file_path.split('').reduce((a, c) => a + c.charCodeAt(0), 0) * 37) % 360;
              const name = f.file_path.split('/').pop() || "";
              const ext = name.includes('.') ? '.' + name.split('.').pop() : "";
              const title = name.replace(ext, "");
              return (
                <button key={f.file_path}
                  onClick={() => handleOpen(f)}
                  title={`${f.file_path}\n${(f.size / 1024).toFixed(1)} KB · ${f.modified ? new Date(f.modified).toLocaleDateString() : ""}`}
                  className="flex flex-col items-center gap-1 group"
                >
                  {/* 书脊卡片 */}
                  <div
                    className="w-14 h-24 rounded-[4px] cursor-pointer transition-all duration-150
                               shadow-sm hover:shadow-md hover:-translate-y-0.5 active:translate-y-0
                               flex flex-col items-center justify-center gap-0.5 px-1"
                    style={{
                      backgroundColor: `hsl(${hue}, 35%, 92%)`,
                      border: `1px solid hsl(${hue}, 40%, 80%)`,
                    }}
                  >
                    {/* 书名（堆叠） */}
                    <span className="text-[9px] font-medium text-center leading-tight text-balance break-words max-w-full"
                      style={{ color: `hsl(${hue}, 50%, 25%)` }}>
                      {title}
                    </span>
                  </div>
                  {/* 扩展名标签 */}
                  <span className="text-[9px] text-muted-foreground">{ext || ".md"}</span>
                </button>
              );
            })}
          </div>
        )}
      </ScrollArea>
    </div>
  );

  return (
    <div className="flex h-full">
      {sidebar}

      {/* 预览对话框 */}
      {preview && (
        <Dialog open onOpenChange={() => setPreview(null)}>
          <DialogContent className="max-w-3xl max-h-[85vh] flex flex-col">
            <DialogHeader>
              <DialogTitle className="text-sm flex items-center gap-2">
                <FileCode className="w-4 h-4" />
                {preview.file_path}
              </DialogTitle>
            </DialogHeader>
            <div className="flex-1 overflow-hidden min-h-0">
              {previewLoading ? (
                <p className="text-xs text-muted-foreground p-2">加载中...</p>
              ) : editing ? (
                <Textarea value={previewContent} onChange={(e) => setPreviewContent(e.target.value)}
                  className="h-full min-h-[300px] font-mono text-xs" />
              ) : (
                <ScrollArea className="h-full max-h-[50vh]">
                  <pre className="text-xs whitespace-pre-wrap font-mono p-2">{previewContent}</pre>
                </ScrollArea>
              )}
            </div>
            <DialogFooter className="gap-2">
              {editing ? (
                <>
                  <Button variant="outline" size="sm" onClick={() => setEditing(false)} className="text-xs">取消</Button>
                  <Button size="sm" onClick={handleSave} disabled={saving} className="text-xs">
                    {saving ? "保存中..." : "保存"}
                  </Button>
                </>
              ) : (
                <>
                  <Button variant="outline" size="sm" onClick={() => setEditing(true)} className="text-xs">
                    <Pencil className="w-3 h-3 mr-1" />编辑
                  </Button>
                  <Button variant="destructive" size="sm" onClick={() => handleDelete(preview)} className="text-xs">
                    <Trash2 className="w-3 h-3 mr-1" />删除
                  </Button>
                </>
              )}
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}

      {content}
    </div>
  );
});
