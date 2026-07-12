import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { BookOpen, BrainCircuit, ChartNoAxesCombined, FileText, PanelLeft, PlugZap, Settings2 } from "lucide-react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/components/ui/sheet";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKBStore } from "@/stores/kbStore";
import { useTheme } from "@/hooks/useTheme";
import { cn } from "@/lib/utils";
import type { SettingsTab } from "@/types/theme";

// 面板按需加载，避免独立设置窗口提前加载所有功能代码。
const GeneralSettings = lazy(() => import("./GeneralSettings").then((module) => ({ default: module.GeneralSettings })));
const LogsPanel = lazy(() => import("./LogsPanel").then((module) => ({ default: module.LogsPanel })));
const KnowledgePanel = lazy(() => import("./KnowledgePanel").then((module) => ({ default: module.KnowledgePanel })));
const MemoryLabPanel = lazy(() => import("./MemoryLabPanel").then((module) => ({ default: module.MemoryLabPanel })));
const StatsPanel = lazy(() => import("./StatsPanel").then((module) => ({ default: module.StatsPanel })));
const PluginsPanel = lazy(() => import("./PluginsPanel").then((module) => ({ default: module.PluginsPanel })));

const navigation: { tab: SettingsTab; label: string; description: string; icon: typeof Settings2 }[] = [
  { tab: "settings", label: "通用设置", description: "模型、连接与外观", icon: Settings2 },
  { tab: "knowledge", label: "知识库", description: "文档与检索来源", icon: BookOpen },
  { tab: "memory", label: "记忆 Lab", description: "长期记忆管理", icon: BrainCircuit },
  { tab: "plugins", label: "插件", description: "工具与扩展能力", icon: PlugZap },
  { tab: "stats", label: "统计", description: "使用情况与成本", icon: ChartNoAxesCombined },
  { tab: "logs", label: "日志", description: "运行诊断信息", icon: FileText },
];

function SettingsNavigation({ tab, onSelect, className }: { tab: SettingsTab; onSelect: (tab: SettingsTab) => void; className?: string }) {
  return (
    <nav aria-label="设置导航" className={cn("flex gap-1", className)}>
      {navigation.map(({ tab: itemTab, label, description, icon: Icon }) => (
        <button
          key={itemTab}
          type="button"
          aria-current={tab === itemTab ? "page" : undefined}
          onClick={() => onSelect(itemTab)}
          className={cn(
            "group flex min-w-0 items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            tab === itemTab
              ? "bg-primary text-primary-foreground shadow-sm"
              : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
          )}
        >
          <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
          <span className="min-w-0">
            <span className="block truncate text-sm font-medium">{label}</span>
            <span className={cn("mt-0.5 block truncate text-xs", tab === itemTab ? "text-primary-foreground/75" : "text-muted-foreground")}>{description}</span>
          </span>
        </button>
      ))}
    </nav>
  );
}

function SettingsPanel({ tab }: { tab: SettingsTab }) {
  const panel = useMemo(() => {
    switch (tab) {
      case "knowledge":
        return <KnowledgePanel />;
      case "memory":
        return <MemoryLabPanel />;
      case "plugins":
        return <PluginsPanel />;
      case "stats":
        return <StatsPanel />;
      case "logs":
        return <LogsPanel active />;
      case "settings":
      default:
        return <GeneralSettings />;
    }
  }, [tab]);

  return (
    <Suspense fallback={<div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">正在加载设置面板…</div>}>
      {panel}
    </Suspense>
  );
}

/** 独立设置窗口（由主窗口 openSettingsWindow 创建，加载 index.html#settings）。 */
export function SettingsWindow() {
  const [tab, setTab] = useState<SettingsTab>("settings");
  const [navigationOpen, setNavigationOpen] = useState(false);
  useTheme(); // 应用主题（深色模式），localStorage 同源共享

  // 加载本窗口的数据（独立 JS 上下文，store 不与主窗口共享）
  useEffect(() => {
    useSettingsStore.getState().load();
    useKBStore.getState().loadKBs();
  }, []);

  // 设置/知识库变更时通知主窗口刷新——两窗口是独立 JS 上下文。
  useEffect(() => {
    const emitChange = () => { void emit("ripple:settings-changed"); };
    const unsubscribeSettings = useSettingsStore.subscribe(emitChange);
    const unsubscribeKB = useKBStore.subscribe(emitChange);
    return () => { unsubscribeSettings(); unsubscribeKB(); };
  }, []);

  // 子 Dialog/Popover 应先处理 Escape；只有无覆盖层时才关闭设置窗口。
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || (event.target as Element | null)?.closest('[role="dialog"]')) return;
      getCurrentWindow().close().catch((error: unknown) => console.error("无法关闭设置窗口", error));
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const closeDesktopNavigation = () => {
      if (window.matchMedia("(min-width: 768px)").matches) setNavigationOpen(false);
    };
    window.addEventListener("resize", closeDesktopNavigation);
    closeDesktopNavigation();
    return () => window.removeEventListener("resize", closeDesktopNavigation);
  }, []);

  const activeItem = navigation.find((item) => item.tab === tab) ?? navigation[0];

  const selectTab = (nextTab: SettingsTab) => {
    setTab(nextTab);
    setNavigationOpen(false);
  };

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-[100dvh] min-h-0 flex-col overflow-hidden bg-background text-foreground">
        <div className="flex h-8 shrink-0 items-center border-b border-border bg-glass px-3" data-tauri-drag-region>
          <span className="text-xs font-medium text-muted-foreground">Ripple 设置</span>
        </div>

        <div className="flex min-h-0 flex-1 overflow-hidden">
          <aside className="hidden w-64 shrink-0 border-r border-border bg-card/50 p-3 md:block">
            <SettingsNavigation tab={tab} onSelect={selectTab} className="flex-col" />
          </aside>

          <Sheet open={navigationOpen} onOpenChange={setNavigationOpen}>
            <SheetContent side="left" className="p-3">
              <SheetTitle className="sr-only">设置导航</SheetTitle>
              <SheetDescription className="sr-only">选择需要配置的设置类别</SheetDescription>
              <SettingsNavigation tab={tab} onSelect={selectTab} className="mt-9 flex-col" />
            </SheetContent>
          </Sheet>

          <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <div className="flex shrink-0 items-center gap-3 border-b border-border px-4 py-3 sm:px-6">
              <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={() => setNavigationOpen(true)} aria-label="打开设置导航">
                <PanelLeft className="h-4 w-4" />
              </Button>
              <div className="min-w-0">
                <h1 className="truncate text-base font-semibold tracking-tight">{activeItem.label}</h1>
                <p className="truncate text-xs text-muted-foreground">{activeItem.description}</p>
              </div>
            </div>
            <section className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
              <SettingsPanel tab={tab} />
            </section>
          </main>
        </div>
      </div>
    </TooltipProvider>
  );
}
