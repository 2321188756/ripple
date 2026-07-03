import { Download, Sun, Moon, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ModelSelector } from "@/components/common/ModelSelector";
import { useSettingsStore } from "@/stores/settingsStore";
import { exportService } from "@/services/export.service";
import type { Theme } from "@/types/theme";

interface ChatHeaderProps {
  activeId: string | null;
  hasMessages: boolean;
  onExportError: (msg: string) => void;
  theme: Theme;
  onThemeChange: (t: Theme) => void;
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
}: ChatHeaderProps) {
  const defaultModel = useSettingsStore((s) => s.defaultModel);
  const setDefaultModel = useSettingsStore((s) => s.setDefaultModel);
  const CurrentIcon = themeLabels[theme].icon;

  const handleExport = async () => {
    if (!activeId) return;
    try {
      const md = await exportService.exportConversation(activeId, "markdown");
      const blob = new Blob([md], { type: "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `chat-${activeId.slice(0, 8)}.md`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      onExportError(String(e));
    }
  };

  return (
    <header className="h-12 border-b border-border flex items-center px-4 gap-2.5 bg-gradient-to-r from-background via-background to-primary/3">
      {/* Logo */}
      <div className="flex items-center gap-2 mr-1">
        <div className="w-6 h-6 rounded-lg bg-primary flex items-center justify-center shadow-sm">
          <span className="text-primary-foreground text-[10px] font-bold">R</span>
        </div>
        <h1 className="text-sm font-semibold tracking-tight">Ripple</h1>
      </div>

      <div className="flex-1" />

      {activeId && hasMessages && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={handleExport}
              aria-label="导出为 Markdown"
            >
              <Download className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>导出 Markdown</TooltipContent>
        </Tooltip>
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
        <DropdownMenuContent align="end" className="w-32">
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
        </DropdownMenuContent>
      </DropdownMenu>

      {activeId && (
        <ModelSelector
          value={hasMessages ? "selected" : defaultModel}
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
