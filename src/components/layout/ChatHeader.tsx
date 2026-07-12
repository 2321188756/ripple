import { Download, Sun, Moon, Monitor, Settings, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ModelSelector } from "@/components/common/ModelSelector";
import { AppLogo } from "@/components/common/AppLogo";
import { useSettingsStore } from "@/stores/settingsStore";
import { exportService } from "@/services/export.service";
import { openSettingsWindow } from "@/lib/openSettings";
import type { Theme } from "@/types/theme";

interface ChatHeaderProps {
  activeId: string | null;
  hasMessages: boolean;
  onExportError: (msg: string) => void;
  theme: Theme;
  onThemeChange: (t: Theme) => void;
  onOpenWorkshop?: () => void;
  isDark: boolean;
}

const themeLabels: Record<Theme, { label: string; icon: typeof Sun }> = {
  light: { label: "浅色", icon: Sun },
  dark: { label: "深色", icon: Moon },
  system: { label: "跟随系统", icon: Monitor },
};

/** 顶部标题栏：标题 + 导出 + 主题切换 + 模型选择 */
export function ChatHeader({
  activeId,
  hasMessages,
  onExportError,
  theme,
  onThemeChange,
  onOpenWorkshop,
}: ChatHeaderProps) {
  const defaultModel = useSettingsStore((s) => s.defaultModel);
  const setDefaultModel = useSettingsStore((s) => s.setDefaultModel);
  const CurrentIcon = themeLabels[theme].icon;

  const handleExport = async (format: "markdown" | "json") => {
    if (!activeId) return;
    try {
      const content = await exportService.exportConversation(activeId, format);
      const blob = new Blob([content], { type: format === "json" ? "application/json" : "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `chat-${activeId.slice(0, 8)}.${format === "json" ? "json" : "md"}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      onExportError(String(e));
    }
  };

  return (
    <header className="h-12 border-b border-border flex items-center px-4 gap-2.5 bg-glass">
      {/* Logo */}
      <div className="flex items-center gap-2 mr-1">
        <AppLogo size="md" />
        <h1 className="text-sm font-semibold tracking-tight">Ripple</h1>
      </div>

      <div className="flex-1" />

      {activeId && hasMessages && (
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="导出">
                  <Download className="w-3.5 h-3.5" />
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>导出</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end" className="w-36">
            <DropdownMenuItem onClick={() => handleExport("markdown")}>Markdown</DropdownMenuItem>
            <DropdownMenuItem onClick={() => handleExport("json")}>JSON（备份用）</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="切换主题">
                <CurrentIcon className="h-3.5 w-3.5" />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>主题</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end" className="w-40">
          {/* 模式切换 */}
          <div className="px-2 py-1 text-[10px] text-muted-foreground font-medium">模式</div>
          {(Object.entries(themeLabels) as [Theme, typeof themeLabels["light"]][]).map(
            ([key, { label, icon: Icon }]) => (
              <DropdownMenuItem
                key={key}
                onClick={() => onThemeChange(key)}
                className="text-xs gap-2"
              >
                <Icon className="w-3.5 h-3.5" />
                {label}
                {theme === key && <span className="ml-auto text-primary">✓</span>}
              </DropdownMenuItem>
            ),
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={onOpenWorkshop} className="text-xs gap-2 text-primary">
            <Sparkles className="w-3.5 h-3.5" />
            主题工坊
          </DropdownMenuItem>
          <DropdownMenuItem onClick={openSettingsWindow} className="text-xs gap-2 text-muted-foreground">
            <Settings className="w-3.5 h-3.5" />
            更多设置
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {activeId && (
        <ModelSelector
          value={defaultModel}
          onChange={async (model) => {
            await setDefaultModel(model);
            if (activeId) {
              try {
                const { conversationService } = await import("@/services");
                await conversationService.update(activeId, { modelId: model });
              } catch (e) {
                onExportError(String(e));
              }
            }
          }}
        />
      )}
    </header>
  );
}
