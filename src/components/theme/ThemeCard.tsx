import { memo } from "react";
import { Check, MoreVertical, Download, Trash2, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import type { ThemeDefinition } from "@/types/theme";

interface ThemeCardProps {
  theme: ThemeDefinition;
  isActive: boolean;
  isDark: boolean;
  onApply: (theme: ThemeDefinition) => void;
  onExport: (theme: ThemeDefinition) => void;
  onDelete: (theme: ThemeDefinition) => void;
}

/** 主题卡片：色块预览 + 名称 + 应用按钮 + 更多菜单 */
export const ThemeCard = memo(function ThemeCard({
  theme, isActive, isDark, onApply, onExport, onDelete,
}: ThemeCardProps) {
  const palette = isDark ? theme.colors.dark : theme.colors.light;
  const hsl = (key: string) => `hsl(${palette[key] || "0 0% 50%"})`;
  const isBuiltin = theme.isBuiltin;

  return (
    <div
      className={cn(
        "rounded-xl border bg-card overflow-hidden transition-all duration-200 hover:shadow-md hover:-translate-y-0.5",
        isActive ? "border-warning ring-2 ring-warning/40" : "border-border",
      )}
    >
      {/* 色块预览：模拟 App 界面 */}
      <div className="relative h-28 p-2.5" style={{ background: hsl("--background") }}>
        {/* 模拟侧边栏 */}
        <div className="absolute left-0 top-0 bottom-0 w-1/4" style={{ background: hsl("--sidebar-background") }} />
        {/* 模拟消息气泡 */}
        <div className="ml-[28%] space-y-1.5">
          <div className="h-3 w-3/4 rounded-full" style={{ background: hsl("--muted") }} />
          <div className="h-3 w-1/2 rounded-full" style={{ background: hsl("--card"), border: `1px solid ${hsl("--border")}` }} />
          <div className="h-3 w-2/3 rounded-full" style={{ background: hsl("--primary"), opacity: 0.9 }} />
        </div>
        {/* 内置角标 */}
        {isBuiltin && (
          <span className="absolute top-1.5 right-1.5 text-[9px] px-1.5 py-0.5 rounded bg-muted/80 text-muted-foreground">
            内置
          </span>
        )}
        {/* 当前使用标记 */}
        {isActive && (
          <div className="absolute bottom-1.5 right-1.5 w-5 h-5 rounded-full bg-warning flex items-center justify-center">
            <Check className="w-3 h-3 text-warning-foreground" />
          </div>
        )}
      </div>

      {/* 底部信息 + 操作 */}
      <div className="p-2.5 flex items-center gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-xs font-medium truncate">{theme.name}</div>
          {theme.description && (
            <div className="text-[10px] text-muted-foreground truncate">{theme.description}</div>
          )}
        </div>
        {!isActive && (
          <Button size="sm" variant="outline" className="h-6 text-[10px] px-2"
            onClick={() => onApply(theme)}>
            应用
          </Button>
        )}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="icon" variant="ghost" className="h-6 w-6 shrink-0">
              <MoreVertical className="w-3 h-3" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => onExport(theme)}>
              <Download className="w-3 h-3 mr-2" /> 导出
            </DropdownMenuItem>
            <DropdownMenuItem disabled>
              <Pencil className="w-3 h-3 mr-2" /> 编辑
            </DropdownMenuItem>
            {!isBuiltin && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem className="text-destructive" onClick={() => onDelete(theme)}>
                  <Trash2 className="w-3 h-3 mr-2" /> 删除
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
});
