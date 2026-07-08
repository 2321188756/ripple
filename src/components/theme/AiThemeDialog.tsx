import { useCallback, useState } from "react";
import { Sparkles, ArrowLeft, Save } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { themeService } from "@/services/theme.service";
import { useTheme } from "@/hooks/useTheme";
import { validateThemeContrast } from "@/lib/theme-contrast";
import type { ThemeDefinition } from "@/types/theme";

const KEYWORD_GROUPS = [
  { label: "自然", words: ["雨后的森林", "温暖拿铁", "樱花飘落", "深海暗流"] },
  { label: "氛围", words: ["赛博朋克", "极简留白", "复古胶片", "星空夜幕"] },
  { label: "情感", words: ["薄荷清晨", "落日余晖", "冰川极光", "暗夜玫瑰"] },
];

interface AiThemeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}

/** AI 主题生成浮层：关键词选择 → LLM 生成 3 套候选 → 保存到主题库。 */
export function AiThemeDialog({ open, onOpenChange, onSaved }: AiThemeDialogProps) {
  const [candidates, setCandidates] = useState<ThemeDefinition[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [keyword, setKeyword] = useState("");
  const { isDark } = useTheme();

  const handleGenerate = useCallback(async (kw: string) => {
    setKeyword(kw);
    setLoading(true);
    setError("");
    setCandidates([]);
    try {
      setCandidates(await themeService.generate(kw));
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }, []);

  const handleSave = useCallback(async (theme: ThemeDefinition) => {
    try {
      // 保存前校验对比度，自动调整不达标的前景色
      const { theme: fixed, warnings } = validateThemeContrast(theme);
      if (warnings.length) {
        console.info("主题对比度已自动调整:", warnings);
      }
      const all = await themeService.list();
      const idx = all.findIndex((t) => t.id === fixed.id);
      if (idx >= 0) all[idx] = fixed; else all.push(fixed);
      await themeService.saveAll(all);
      onSaved();
      onOpenChange(false);
      setCandidates([]);
      setKeyword("");
    } catch (e) { setError(`保存失败: ${e}`); }
  }, [onSaved, onOpenChange]);

  const handleBack = () => { setCandidates([]); setKeyword(""); setError(""); };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-base">
            <Sparkles className="w-4 h-4 text-primary" />
            AI 主题设计
            {keyword && !loading && candidates.length > 0 && (
              <Button variant="ghost" size="sm" className="h-6 text-xs ml-auto" onClick={handleBack}>
                <ArrowLeft className="w-3 h-3 mr-1" /> 换关键词
              </Button>
            )}
          </DialogTitle>
        </DialogHeader>

        <ScrollArea className="flex-1 min-h-0">
          <div className="p-1">
            {loading ? (
              <div className="flex flex-col items-center justify-center py-16 gap-4">
                <Sparkles className="w-8 h-8 text-primary animate-pulse" />
                <p className="text-sm text-muted-foreground">正在为「{keyword}」设计配色...</p>
                <div className="flex gap-2">
                  {[0, 1, 2].map((i) => (
                    <div key={i} className="w-16 h-16 rounded-lg bg-muted animate-pulse"
                      style={{ animationDelay: `${i * 150}ms` }} />
                  ))}
                </div>
              </div>
            ) : candidates.length > 0 ? (
              <div className="space-y-3">
                <p className="text-xs text-muted-foreground text-center">
                  已生成 {candidates.length} 套候选，点击保存加入主题库
                </p>
                {candidates.map((t) => (
                  <CandidateCard key={t.id} theme={t} isDark={isDark} onSave={handleSave} />
                ))}
              </div>
            ) : (
              <div className="space-y-5 py-2">
                <p className="text-xs text-muted-foreground text-center">选择一个关键词，让 AI 为你生成主题</p>
                {KEYWORD_GROUPS.map((g) => (
                  <div key={g.label}>
                    <div className="text-xs font-medium text-muted-foreground mb-2">{g.label}</div>
                    <div className="flex flex-wrap gap-2">
                      {g.words.map((w) => (
                        <button key={w} onClick={() => handleGenerate(w)}
                          className="px-3 py-1.5 rounded-full text-xs border border-border bg-card hover:bg-accent hover:border-primary/40 transition-colors">
                          {w}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
                {error && <div className="text-xs text-destructive text-center pt-2">{error}</div>}
              </div>
            )}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}

/** 候选主题卡片：色块预览 + 名称 + 保存按钮 */
function CandidateCard({ theme, isDark, onSave }: {
  theme: ThemeDefinition;
  isDark: boolean;
  onSave: (t: ThemeDefinition) => void;
}) {
  const palette = isDark ? theme.colors.dark : theme.colors.light;
  const hsl = (k: string) => `hsl(${palette[k] || "0 0% 50%"})`;
  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden flex">
      {/* 色块预览 */}
      <div className="relative w-32 h-20 shrink-0" style={{ background: hsl("--background") }}>
        <div className="absolute left-0 top-0 bottom-0 w-1/3" style={{ background: hsl("--sidebar-background") }} />
        <div className="absolute left-[40%] top-3 right-2 space-y-1.5">
          <div className="h-2 w-full rounded-full" style={{ background: hsl("--muted") }} />
          <div className="h-2 w-2/3 rounded-full" style={{ background: hsl("--card"), border: `1px solid ${hsl("--border")}` }} />
          <div className="h-2 w-1/2 rounded-full" style={{ background: hsl("--primary") }} />
        </div>
      </div>
      {/* 信息 + 保存 */}
      <div className="flex-1 p-3 flex flex-col justify-between min-w-0">
        <div className="min-w-0">
          <div className="text-sm font-medium truncate">{theme.name}</div>
          {theme.description && <div className="text-[10px] text-muted-foreground truncate">{theme.description}</div>}
        </div>
        <Button size="sm" className="h-6 text-[10px] w-full" onClick={() => onSave(theme)}>
          <Save className="w-3 h-3 mr-1" /> 保存到主题库
        </Button>
      </div>
    </div>
  );
}
