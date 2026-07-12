import { useCallback, useEffect, useState } from "react";
import { Sparkles, ArrowLeft, Save, Wand2, FileEdit, RotateCcw } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ModelSelector } from "@/components/common/ModelSelector";
import { themeService } from "@/services/theme.service";
import { useTheme } from "@/hooks/useTheme";
import { useSettingsStore } from "@/stores/settingsStore";
import { validateThemeContrast } from "@/lib/theme-contrast";
import { loadThemePrompt, saveThemePrompt, buildPrompt, DEFAULT_THEME_PROMPT } from "@/lib/theme-prompt";
import type { ThemeDefinition } from "@/types/theme";

const INSPIRATIONS = [
  "雨后的森林", "温暖拿铁", "赛博朋克", "极简留白",
  "樱花飘落", "深海暗流", "复古胶片", "星空夜幕",
  "薄荷清晨", "落日余晖", "冰川极光", "暗夜玫瑰",
];

interface AiThemeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}

/** AI 主题生成浮层：需求描述 + 模型选择 + prompt 编辑 + 可选图片取色。 */
export function AiThemeDialog({ open, onOpenChange, onSaved }: AiThemeDialogProps) {
  const [userInput, setUserInput] = useState("");
  const [model, setModel] = useState(() => useSettingsStore.getState().defaultModel || "deepseek-v4-flash");
  const [promptTemplate, setPromptTemplate] = useState(DEFAULT_THEME_PROMPT);
  const [promptEditorOpen, setPromptEditorOpen] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState("");
  const [candidates, setCandidates] = useState<ThemeDefinition[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const { isDark } = useTheme();

  // 加载用户自定义 prompt 模板
  useEffect(() => {
    if (open) loadThemePrompt().then(setPromptTemplate).catch(() => {});
  }, [open]);

  const handleGenerate = useCallback(async () => {
    if (!userInput.trim()) { setError("请先描述你的主题需求"); return; }
    setLoading(true);
    setError("");
    setCandidates([]);
    try {
      const fullPrompt = buildPrompt(promptTemplate, userInput.trim());
      setCandidates(await themeService.generate(fullPrompt, model));
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }, [userInput, promptTemplate, model]);

  const handleSave = useCallback(async (theme: ThemeDefinition) => {
    try {
      const { theme: fixed, warnings } = validateThemeContrast(theme);
      if (warnings.length) console.info("主题对比度已自动调整:", warnings);
      const all = await themeService.list();
      const idx = all.findIndex((t) => t.id === fixed.id);
      if (idx >= 0) all[idx] = fixed; else all.push(fixed);
      await themeService.saveAll(all);
      onSaved();
      onOpenChange(false);
      setCandidates([]);
      setUserInput("");
    } catch (e) { setError(`保存失败: ${e}`); }
  }, [onSaved, onOpenChange]);

  const handleBack = () => { setCandidates([]); setError(""); };

  const openPromptEditor = () => {
    setEditingTemplate(promptTemplate);
    setPromptEditorOpen(true);
  };

  const savePrompt = async () => {
    await saveThemePrompt(editingTemplate);
    setPromptTemplate(editingTemplate);
    setPromptEditorOpen(false);
  };

  const resetPrompt = () => setEditingTemplate(DEFAULT_THEME_PROMPT);

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Sparkles className="w-4 h-4 text-primary" />
              AI 主题设计
              {candidates.length > 0 && !loading && (
                <Button variant="ghost" size="sm" className="h-6 text-xs ml-auto" onClick={handleBack}>
                  <ArrowLeft className="w-3 h-3 mr-1" /> 重新描述
                </Button>
              )}
            </DialogTitle>
          </DialogHeader>

          <ScrollArea className="flex-1 min-h-0">
            <div className="p-1">
              {loading ? (
                <div className="flex flex-col items-center justify-center py-16 gap-4">
                  <Sparkles className="w-8 h-8 text-primary animate-pulse" />
                  <p className="text-sm text-muted-foreground">正在用 {model} 设计主题...</p>
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
                <div className="space-y-4 py-2">
                  {/* 模型选择 + prompt 编辑 */}
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-muted-foreground shrink-0">设计模型</span>
                    <ModelSelector value={model} onChange={setModel} />
                    <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0"
                      onClick={openPromptEditor} title="编辑 AI 预设词">
                      <FileEdit className="w-3.5 h-3.5" />
                    </Button>
                  </div>

                  {/* 需求描述 */}
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium">描述你想要的主题</label>
                    <Textarea
                      value={userInput}
                      onChange={(e) => setUserInput(e.target.value)}
                      rows={3}
                      placeholder="如：适合深夜写代码的暗色主题，带点赛博朋克霓虹感，主色偏青蓝"
                      className="text-xs resize-none"
                    />
                  </div>

                  {/* 灵感词（点击填入） */}
                  <div>
                    <div className="text-[11px] text-muted-foreground mb-2">灵感（点击填入描述）</div>
                    <div className="flex flex-wrap gap-1.5">
                      {INSPIRATIONS.map((w) => (
                        <button key={w} onClick={() => setUserInput(w)}
                          className="px-2.5 py-1 rounded-full text-[11px] border border-border bg-card hover:bg-accent hover:border-primary/40 transition-colors">
                          {w}
                        </button>
                      ))}
                    </div>
                  </div>

                  {error && <div className="text-xs text-destructive">{error}</div>}

                  <Button className="w-full h-8 text-xs" onClick={handleGenerate} disabled={!userInput.trim()}>
                    <Wand2 className="w-3.5 h-3.5 mr-1.5" /> 生成主题
                  </Button>
                </div>
              )}
            </div>
          </ScrollArea>
        </DialogContent>
      </Dialog>

      {/* Prompt 编辑器 */}
      <Dialog open={promptEditorOpen} onOpenChange={setPromptEditorOpen}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="text-sm flex items-center gap-2">
              <FileEdit className="w-4 h-4" /> AI 预设词编辑
            </DialogTitle>
          </DialogHeader>
          <div className="flex-1 min-h-0 flex flex-col gap-2">
            <p className="text-[11px] text-muted-foreground">
              <code className="text-warning">__KW__</code> 会被替换为你的需求描述。修改后保存即生效。
            </p>
            <Textarea
              value={editingTemplate}
              onChange={(e) => setEditingTemplate(e.target.value)}
              className="flex-1 min-h-[300px] text-[11px] font-mono resize-none"
            />
          </div>
          <div className="flex justify-between items-center pt-2 border-t border-border">
            <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={resetPrompt}>
              <RotateCcw className="w-3 h-3 mr-1" /> 恢复默认
            </Button>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => setPromptEditorOpen(false)}>取消</Button>
              <Button size="sm" className="h-7 text-xs" onClick={savePrompt}>
                <Save className="w-3 h-3 mr-1" /> 保存
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
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
      <div className="relative w-32 h-20 shrink-0" style={{ background: hsl("--background") }}>
        <div className="absolute left-0 top-0 bottom-0 w-1/3" style={{ background: hsl("--sidebar-background") }} />
        <div className="absolute left-[40%] top-3 right-2 space-y-1.5">
          <div className="h-2 w-full rounded-full" style={{ background: hsl("--muted") }} />
          <div className="h-2 w-2/3 rounded-full" style={{ background: hsl("--card"), border: `1px solid ${hsl("--border")}` }} />
          <div className="h-2 w-1/2 rounded-full" style={{ background: hsl("--primary") }} />
        </div>
      </div>
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
