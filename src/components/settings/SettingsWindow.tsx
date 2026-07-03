import { lazy, Suspense, useEffect, useState } from "react";
import { X } from "lucide-react";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKBStore } from "@/stores/kbStore";
import { useTheme } from "@/hooks/useTheme";
import type { SettingsTab } from "@/types/theme";

// 面板懒加载，减小设置窗口首屏负担
const GeneralSettings = lazy(() => import("./GeneralSettings").then((m) => ({ default: m.GeneralSettings })));
const LogsPanel = lazy(() => import("./LogsPanel").then((m) => ({ default: m.LogsPanel })));
const KnowledgePanel = lazy(() => import("./KnowledgePanel").then((m) => ({ default: m.KnowledgePanel })));
const StatsPanel = lazy(() => import("./StatsPanel").then((m) => ({ default: m.StatsPanel })));
const PluginsPanel = lazy(() => import("./PluginsPanel").then((m) => ({ default: m.PluginsPanel })));

/** 独立设置窗口（由主窗口 openSettingsWindow 创建，加载 index.html#settings）。 */
export function SettingsWindow() {
  const [tab, setTab] = useState<SettingsTab>("settings");
  useTheme(); // 应用主题（深色模式），localStorage 同源共享

  // 加载本窗口的数据（独立 JS 上下文，store 不与主窗口共享）
  useEffect(() => {
    useSettingsStore.getState().load();
    useKBStore.getState().loadKBs();
  }, []);

  // 设置/知识库变更时通知主窗口刷新——两窗口是独立 JS 上下文，
  // 不通知的话主窗口缓存的 apiKey/KB 列表会过期
  useEffect(() => {
    const emitChange = () => { void emit("ripple:settings-changed"); };
    const u1 = useSettingsStore.subscribe(emitChange);
    const u2 = useKBStore.subscribe(emitChange);
    return () => { u1(); u2(); };
  }, []);

  // Escape 关闭窗口
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") getCurrentWindow().close().catch(() => {});
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const close = () => getCurrentWindow().close().catch(() => {});

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex flex-col h-screen bg-background text-foreground">
        {/* 标题栏：data-tauri-drag-region 让系统原生拖动窗口 */}
        <div
          className="flex items-center justify-between px-5 py-3 border-b border-border bg-muted/30"
          data-tauri-drag-region
        >
          <span className="text-sm font-semibold">设置</span>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={close} aria-label="关闭">
            <X className="w-4 h-4" />
          </Button>
        </div>

        <Tabs
          value={tab}
          onValueChange={(v) => setTab(v as SettingsTab)}
          className="flex-1 flex flex-col min-h-0"
        >
          <div className="px-5 pt-3 pb-0">
            <TabsList className="w-full justify-start gap-1">
              <TabsTrigger value="settings">通用</TabsTrigger>
              <TabsTrigger value="knowledge">知识库</TabsTrigger>
              <TabsTrigger value="plugins">插件</TabsTrigger>
              <TabsTrigger value="stats">统计</TabsTrigger>
              <TabsTrigger value="logs">日志</TabsTrigger>
            </TabsList>
          </div>

          <div className="flex-1 overflow-hidden">
            <Suspense fallback={<div className="p-5 text-xs text-muted-foreground">加载中...</div>}>
              <TabsContent value="settings" className="mt-0 h-full overflow-y-auto p-5">
                <GeneralSettings />
              </TabsContent>
              <TabsContent value="knowledge" className="mt-0 h-full overflow-y-auto p-5">
                <KnowledgePanel />
              </TabsContent>
              <TabsContent value="plugins" className="mt-0 h-full overflow-y-auto p-5">
                <PluginsPanel />
              </TabsContent>
              <TabsContent value="stats" className="mt-0 h-full overflow-y-auto p-5">
                <StatsPanel />
              </TabsContent>
              <TabsContent value="logs" className="mt-0 h-full flex flex-col min-h-0 p-5">
                <LogsPanel active={tab === "logs"} />
              </TabsContent>
            </Suspense>
          </div>
        </Tabs>
      </div>
    </TooltipProvider>
  );
}
