import { useCallback, useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Sparkles, Upload, X } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { ThemeCard } from "./ThemeCard";
import { AiThemeDialog } from "./AiThemeDialog";
import { themeService, prepareThemeForSave } from "@/services/theme.service";
import { useTheme } from "@/hooks/useTheme";
import type { ThemeDefinition } from "@/types/theme";


interface ThemeWorkshopProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** 主题工坊：浮层面板，管理主题的浏览/应用/导入/导出/删除。AI 生成在 Phase 2 接入。 */
export function ThemeWorkshop({ open, onOpenChange }: ThemeWorkshopProps) {
  const [themes, setThemes] = useState<ThemeDefinition[]>([]);
  const [loading, setLoading] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const { applyCustomTheme, isDark, activeThemeId, refreshActiveTheme } = useTheme();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setThemes(await themeService.list());
    } catch (e) { console.error(e); }
    setLoading(false);
  }, []);

  useEffect(() => { if (open) load(); }, [open, load]);

  /** 从文件路径导入主题（拖拽和文件选择器共用） */
  const importFromPath = useCallback(async (path: string) => {
    if (!/\.(theme|json)$/i.test(path)) {
      alert("仅支持 .theme 或 .json 文件");
      return;
    }
    try {
      const imported = await themeService.importTheme(path);
      const { warnings } = prepareThemeForSave(imported);
      if (warnings.length) alert(warnings.map((warning) => warning.message).join("\n"));
      await load();
    } catch (e) { alert(`导入失败: ${e}`); }
  }, [load]);

  // 拖拽导入：窗口打开时监听 webview 拖放事件
  useEffect(() => {
    if (!open) return;
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragOver(true);
      } else if (event.payload.type === "drop") {
        setDragOver(false);
        const paths = event.payload.paths;
        if (paths && paths.length > 0) importFromPath(paths[0]);
      } else if (event.payload.type === "leave") {
        setDragOver(false);
      }
    });
    return () => { unlisten.then((f) => f()).catch(() => {}); };
  }, [open, importFromPath]);

  const handleApply = useCallback(async (theme: ThemeDefinition) => {
    await applyCustomTheme(theme);
  }, [applyCustomTheme]);

  /** 设置壁纸：选图片 -> 复制到壁纸目录 -> 更新主题 -> 同步显示壁纸 */
  const handleSetWallpaper = useCallback(async (theme: ThemeDefinition) => {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp"] }],
    });
    if (!path) return;
    try {
      const wpPath = await themeService.saveWallpaper(path as string, theme.id);
      const all = await themeService.list();
      const idx = all.findIndex((t) => t.id === theme.id);
      if (idx >= 0) {
        all[idx] = prepareThemeForSave({ ...all[idx], wallpaper: wpPath }).theme;
        await themeService.saveAll(all);
        await load();
        if (theme.id === activeThemeId) {
          await refreshActiveTheme();
        }
      }
    } catch (e) { alert(`设置壁纸失败: ${e}`); }
  }, [activeThemeId, load, refreshActiveTheme]);

  const handleExport = useCallback(async (theme: ThemeDefinition) => {
    const path = await saveDialog({
      defaultPath: `${theme.name}.theme`,
      filters: [{ name: "Theme", extensions: ["theme", "json"] }],
    });
    if (!path) return;
    try { await themeService.exportTheme(theme.id, path); }
    catch (e) { alert(`导出失败: ${e}`); }
  }, []);

  const handleImport = useCallback(async () => {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "Theme", extensions: ["theme", "json"] }],
    });
    if (!path) return;
    await importFromPath(path as string);
  }, [importFromPath]);

  const handleDelete = useCallback(async (theme: ThemeDefinition) => {
    if (theme.isBuiltin) return;
    if (theme.id === activeThemeId) {
      alert("正在使用的主题不可删除，请先切换到其他主题");
      return;
    }
    if (!confirm(`确认删除主题「${theme.name}」？此操作不可撤销。`)) return;
    try {
      await themeService.deleteTheme(theme.id);
      await load();
    } catch (e) { alert(`删除失败: ${e}`); }
  }, [activeThemeId, load]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={cn(
        "max-w-3xl max-h-[85vh] flex flex-col p-0 gap-0 overflow-hidden transition-colors",
        dragOver && "ring-2 ring-primary ring-offset-2 ring-offset-background",
      )}>
        {dragOver && (
          <div className="absolute inset-0 z-50 flex items-center justify-center bg-primary/10 backdrop-blur-sm pointer-events-none">
            <div className="flex flex-col items-center gap-2 text-primary">
              <Upload className="w-10 h-10" />
              <p className="text-sm font-medium">松开以导入主题文件</p>
            </div>
          </div>
        )}
        {/* 顶部操作栏 */}
        <DialogHeader className="px-5 py-3.5 border-b border-border flex-row items-center justify-between space-y-0">
          <DialogTitle className="text-base font-semibold">我的主题</DialogTitle>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" className="h-7 text-xs" onClick={handleImport}>
              <Upload className="w-3.5 h-3.5 mr-1.5" /> 导入
            </Button>
            <Button size="sm" className="h-7 text-xs" onClick={() => setAiOpen(true)}>
              <Sparkles className="w-3.5 h-3.5 mr-1.5" /> 新建 AI 主题
            </Button>
          </div>
        </DialogHeader>

        {/* 主题卡片网格 */}
        <ScrollArea className="flex-1">
          <div className="p-5">
            {loading ? (
              <div className="text-center text-xs text-muted-foreground py-12">加载中...</div>
            ) : themes.length === 0 ? (
              <div className="text-center py-12">
                <Sparkles className="w-10 h-10 mx-auto mb-3 text-muted-foreground/40" />
                <p className="text-sm text-muted-foreground">还没有主题，点击「新建 AI 主题」创建一个吧</p>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-4">
                {themes.map((t) => (
                  <ThemeCard
                    key={t.id}
                    theme={t}
                    isActive={t.id === activeThemeId}
                    isDark={isDark}
                    onApply={handleApply}
                    onExport={handleExport}
                    onDelete={handleDelete}
                    onSetWallpaper={handleSetWallpaper}
                  />
                ))}
              </div>
            )}

            {/* 壁纸阅读层提示：壁纸存在时使用更强的 token 化磨砂与内容洗底，保证可读性。 */}
            {(() => {
              const active = themes.find((t) => t.id === activeThemeId);
              if (!active?.wallpaper) return null;
              return (
                <div className="mt-4 p-3 rounded-lg border border-border bg-card/70">
                  <div className="text-xs font-medium mb-2">壁纸已启用（可读性保护）</div>
                  <p className="text-[10px] text-muted-foreground">
                    侧边栏、头部与输入区会使用 token 化的高不透明度磨砂层；消息区保留独立阅读底色，
                    让壁纸可见但不会干扰文字、边框和焦点状态。
                  </p>
                </div>
              );
            })()}
          </div>
        </ScrollArea>

        {/* 底部关闭 */}
        <div className="px-5 py-3 border-t border-border flex justify-center bg-card/50">
          <Button variant="outline" size="sm" className="h-8 text-xs px-6" onClick={() => onOpenChange(false)}>
            <X className="w-3.5 h-3.5 mr-1.5" /> 关闭
          </Button>
        </div>
      </DialogContent>

      <AiThemeDialog open={aiOpen} onOpenChange={setAiOpen} onSaved={load} />
    </Dialog>
  );
}
