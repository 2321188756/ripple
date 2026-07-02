import { useCallback, useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { GeneralSettings } from "./GeneralSettings";
import { LogsPanel } from "./LogsPanel";
import { KnowledgePanel } from "./KnowledgePanel";
import { StatsPanel } from "./StatsPanel";
import { PluginsPanel } from "./PluginsPanel";
import { useUIStore } from "@/stores/uiStore";
import { SETTINGS_PANEL_KEYS } from "@/lib/constants";
import type { SettingsTab } from "@/types/theme";

const DEFAULT_W = 820;
const DEFAULT_H = 620;

/** 可拖拽、可缩放的设置面板（位置/大小持久化到 localStorage） */
export function SettingsDialog() {
  const { settingsOpen, setSettingsOpen, settingsTab, setSettingsTab } = useUIStore();
  const panelRef = useRef<HTMLDivElement>(null);

  const [panelPos, setPanelPos] = useState(() => ({
    x: parseInt(localStorage.getItem(SETTINGS_PANEL_KEYS.x) || "-1"),
    y: parseInt(localStorage.getItem(SETTINGS_PANEL_KEYS.y) || "-1"),
  }));
  const [panelSize] = useState(() => ({
    w: parseInt(localStorage.getItem(SETTINGS_PANEL_KEYS.w) || String(DEFAULT_W)),
    h: parseInt(localStorage.getItem(SETTINGS_PANEL_KEYS.h) || String(DEFAULT_H)),
  }));

  // 监听面板大小变化并保存
  useEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = Math.round(entry.contentBoxSize[0]?.inlineSize || entry.contentRect.width);
        const h = Math.round(entry.contentBoxSize[0]?.blockSize || entry.contentRect.height);
        if (w > 300 && h > 200) {
          localStorage.setItem(SETTINGS_PANEL_KEYS.w, String(w));
          localStorage.setItem(SETTINGS_PANEL_KEYS.h, String(h));
        }
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const onDragStart = useCallback((e: React.MouseEvent) => {
    const rect = panelRef.current?.getBoundingClientRect();
    if (!rect) return;
    const offX = e.clientX - rect.left;
    const offY = e.clientY - rect.top;
    const onMove = (ev: MouseEvent) => {
      const nx = ev.clientX - offX;
      const ny = ev.clientY - offY;
      setPanelPos({ x: nx, y: ny });
      localStorage.setItem(SETTINGS_PANEL_KEYS.x, String(Math.round(nx)));
      localStorage.setItem(SETTINGS_PANEL_KEYS.y, String(Math.round(ny)));
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp, { once: true });
  }, []);

  if (!settingsOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/40 z-50 animate-in fade-in-0 duration-150">
      <div
        ref={panelRef}
        className="bg-background rounded-2xl shadow-2xl flex flex-col overflow-hidden border border-border animate-in zoom-in-95 fade-in-0 duration-200"
        style={{
          position: "fixed",
          left: panelPos.x < 0 ? "50%" : panelPos.x,
          top: panelPos.y < 0 ? "45%" : panelPos.y,
          width: panelSize.w,
          height: panelSize.h,
          minWidth: 560,
          minHeight: 380,
          resize: "both",
          transform: panelPos.x < 0 ? "translate(-50%,-50%)" : undefined,
        }}
      >
        {/* 标题栏（可拖拽） */}
        <div
          className="flex items-center justify-between px-5 py-3 border-b border-border cursor-move select-none bg-muted/30"
          onMouseDown={onDragStart}
        >
          <span className="text-sm font-semibold">设置</span>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 rounded-lg"
            onClick={() => setSettingsOpen(false)}
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>

        <Tabs
          value={settingsTab}
          onValueChange={(v) => setSettingsTab(v as SettingsTab)}
          className="flex-1 flex flex-col min-h-0"
        >
          <div className="px-5 pt-3 pb-0">
            <TabsList className="w-full justify-start gap-1">
              <TabsTrigger value="settings" className="gap-1.5">
                通用
              </TabsTrigger>
              <TabsTrigger value="knowledge" className="gap-1.5">
                知识库
              </TabsTrigger>
              <TabsTrigger value="plugins" className="gap-1.5">
                插件
              </TabsTrigger>
              <TabsTrigger value="stats" className="gap-1.5">
                统计
              </TabsTrigger>
              <TabsTrigger value="logs" className="gap-1.5">
                日志
              </TabsTrigger>
            </TabsList>
          </div>

          <div className="flex-1 overflow-y-auto p-5">
            <TabsContent value="settings" className="mt-0">
              <GeneralSettings />
            </TabsContent>
            <TabsContent value="knowledge" className="mt-0">
              <KnowledgePanel />
            </TabsContent>
            <TabsContent value="plugins" className="mt-0">
              <PluginsPanel />
            </TabsContent>
            <TabsContent value="stats" className="mt-0">
              <StatsPanel />
            </TabsContent>
            <TabsContent value="logs" className="mt-0">
              <LogsPanel active={settingsTab === "logs"} />
            </TabsContent>
          </div>
        </Tabs>
      </div>
    </div>
  );
}
