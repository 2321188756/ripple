import { useCallback, useEffect, useState } from "react";
import { Sparkles, ArrowLeft, Save, Wand2, FileEdit, RotateCcw } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ModelSelector } from "@/components/common/ModelSelector";
import { prepareThemeForSave, themeService } from "@/services/theme.service";
import { useTheme } from "@/hooks/useTheme";
import { useSettingsStore } from "@/stores/settingsStore";
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
  const [savingId, setSavingId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [validationMessage, setValidationMessage] = useState("");
  const { isDark } = useTheme();

  useEffect(() => {
    if (open) loadThemePrompt().then(setPromptTemplate).catch(() => {});
  }, [open]);

  const handleGenerate = useCallback(async () => {
    if (!userInput.trim()) { setError("请先描述你的主题需求"); return; }
    setLoading(true);
    setError("");
    setValidationMessage("");
    setCandidates([]);
    try {
      const fullPrompt = buildPrompt(promptTemplate, userInput.trim());
      setCandidates(await themeService.generate(fullPrompt, model));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [userInput, promptTemplate, model]);

  const handleSave = useCallback(async (theme: ThemeDefinition) => {
    setSavingId(theme.id);
    setError("");
    try {
      const { theme: fixed, warnings } = prepareThemeForSave(theme);
      const all = await themeService.list();
      const idx = all.findIndex((item) => item.id === fixed.id);
      if (idx >= 0) all[idx] = fixed; else all.push(fixed);
      await themeService.saveAll(all);
      setValidationMessage(warnings.length ? warnings.map((warning) => warning.message).join("；") : "主题已保存到主题库");
      onSaved();
      onOpenChange(false);
      setCandidates([]);
      setUserInput("");
    } catch (e) {
      setError(`保存失败: ${e}`);
    } finally {
      setSavingId(null);
    }
  }, [onSaved, onOpenChange]);

  const handleBack = () => { setCandidates([]); setError(""); setValidationMessage(""); };
  const openPromptEditor = () => { setEditingTemplate(promptTemplate); setPromptEditorOpen(true); };
  const savePrompt = async () => {
    await saveThemePrompt(editingTemplate);
    setPromptTemplate(editingTemplate);
    setPromptEditorOpen(false);
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col" aria-busy={loading || Boolean(savingId)}>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Sparkles className="w-4 h-4 text-primary" aria-hidden="true" />
              AI 主题设计
              {candidates.length > 0 && !loading && (
                <Button variant="ghost" size="sm" className="h-6 text-xs ml-auto" onClick={handleBack} disabled={Boolean(savingId)}>
                  <ArrowLeft className="w-3 h-3 mr-1" aria-hidden="true" /> 重新描述
                </Button>
              )}
            </DialogTitle>
            <DialogDescription className="text-xs">描述想要的视觉风格，生成候选主题后可保存到主题库。</DialogDescription>
          </DialogHeader>

          <ScrollArea className="flex-1 min-h-0">
            <div className="p-1">
              {loading ? (
                <div className="flex flex-col items-center justify-center py-16 gap-4" role="status" aria-live="polite">
                  <Sparkles className="w-8 h-8 text-primary animate-pulse" aria-hidden="true" />
                  <p className="text-sm text-muted-foreground">正在用 {model} 设计主题...</p>
                  <div className="flex gap-2" aria-hidden="true">
                    {[0, 1, 2].map((i) => <div key={i} className="w-16 h-16 rounded-lg bg-muted animate-pulse" style={{ animationDelay: `${i * 150}ms` }} />)}
                  </div>
                </div>
              ) : candidates.length > 0 ? (
                <div className="space-y-3">
                  <p className="text-xs text-muted-foreground text-center">已生成 {candidates.length} 套候选，点击保存加入主题库</p>
                  {candidates.map((theme) => <CandidateCard key={theme.id} theme={theme} isDark={isDark} onSave={handleSave} saving={savingId === theme.id} />)}
                </div>
              ) : (
                <div className="space-y-4 py-2">
                  <div className="flex items-center gap-2">
                    <label htmlFor="theme-model" className="text-xs text-muted-foreground shrink-0">设计模型</label>
                    <ModelSelector id="theme-model" value={model} onChange={setModel} disabled={loading} />
                    <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0" onClick={openPromptEditor} aria-label="编辑 AI 预设词">
                      <FileEdit className="w-3.5 h-3.5" aria-hidden="true" />
                    </Button>
                  </div>
                  <div className="space-y-1.5">
                    <label htmlFor="theme-requirements" className="text-xs font-medium">描述你想要的主题</label>
                    <Textarea id="theme-requirements" value={userInput} onChange={(e) => setUserInput(e.target.value)} rows={3}
                      placeholder="如：适合深夜写代码的暗色主题，带点赛博朋克霓虹感，主色偏青蓝" className="text-xs resize-none" />
                  </div>
                  <div role="group" aria-label="主题灵感">
                    <div className="text-[11px] text-muted-foreground mb-2">灵感（点击填入描述）</div>
                    <div className="flex flex-wrap gap-1.5">
                      {INSPIRATIONS.map((word) => (
                        <button key={word} type="button" onClick={() => setUserInput(word)} className="px-2.5 py-1 rounded-full text-[11px] border border-border bg-card hover:bg-accent hover:border-primary/40 transition-colors">
                          {word}
                        </button>
                      ))}
                    </div>
                  </div>
                  {error && <div className="text-xs text-destructive" role="alert">{error}</div>}
                  {validationMessage && <div className="text-xs text-muted-foreground" role="status" aria-live="polite">{validationMessage}</div>}
                  <Button className="w-full h-8 text-xs" onClick={handleGenerate} disabled={!userInput.trim() || loading}>
                    <Wand2 className="w-3.5 h-3.5 mr-1.5" aria-hidden="true" /> 生成主题
                  </Button>
                </div>
              )}
            </div>
          </ScrollArea>
        </DialogContent>
      </Dialog>

      <Dialog open={promptEditorOpen} onOpenChange={setPromptEditorOpen}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="text-sm flex items-center gap-2"><FileEdit className="w-4 h-4" aria-hidden="true" /> AI 预设词编辑</DialogTitle>
            <DialogDescription className="text-[11px]"><code className="text-warning">__KW__</code> 会被替换为你的需求描述。修改后保存即生效。</DialogDescription>
          </DialogHeader>
          <div className="flex-1 min-h-0 flex flex-col gap-2">
            <label htmlFor="theme-prompt-template" className="sr-only">AI 预设词模板</label>
            <Textarea id="theme-prompt-template" value={editingTemplate} onChange={(e) => setEditingTemplate(e.target.value)} className="flex-1 min-h-[300px] text-[11px] font-mono resize-none" />
          </div>
          <div className="flex justify-between items-center pt-2 border-t border-border">
            <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={() => setEditingTemplate(DEFAULT_THEME_PROMPT)}><RotateCcw className="w-3 h-3 mr-1" aria-hidden="true" /> 恢复默认</Button>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => setPromptEditorOpen(false)}>取消</Button>
              <Button size="sm" className="h-7 text-xs" onClick={savePrompt}><Save className="w-3 h-3 mr-1" aria-hidden="true" /> 保存</Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

function CandidateCard({ theme, isDark, onSave, saving }: {
  theme: ThemeDefinition;
  isDark: boolean;
  onSave: (theme: ThemeDefinition) => void;
  saving: boolean;
}) {
  const palette = isDark ? theme.colors.dark : theme.colors.light;
  const fallback = isDark ? "224 71% 8%" : "240 10% 97%";
  const hsl = (key: string) => `hsl(${palette[key] || palette["--primary"] || fallback})`;
  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden flex">
      <div className="relative w-32 h-20 shrink-0" style={{ background: hsl("--background") }} aria-hidden="true">
        <div className="absolute left-0 top-0 bottom-0 w-1/3" style={{ background: hsl("--sidebar-background") }} />
        <div className="absolute left-[40%] top-3 right-2 space-y-1.5">
          <div className="h-2 w-full rounded-full" style={{ background: hsl("--muted") }} />
          <div className="h-2 w-2/3 rounded-full" style={{ background: hsl("--card"), border: `1px solid ${hsl("--border")}` }} />
          <div className="h-2 w-1/2 rounded-full" style={{ background: `linear-gradient(90deg, ${hsl("--gradient-from")}, ${hsl("--gradient-via")}, ${hsl("--gradient-to")})` }} />
          <div className="flex gap-1 pt-0.5">
            {["--primary", "--accent", "--success", "--warning", "--info"].map((key) => (
              <span key={key} className="h-2 w-2 rounded-full" style={{ background: hsl(key) }} />
            ))}
          </div>
        </div>
      </div>
      <div className="flex-1 p-3 flex flex-col justify-between min-w-0">
        <div className="min-w-0"><div className="text-sm font-medium truncate">{theme.name}</div>{theme.description && <div className="text-[10px] text-muted-foreground truncate">{theme.description}</div>}</div>
        <Button size="sm" className="h-6 text-[10px] w-full" onClick={() => onSave(theme)} disabled={saving} aria-label={`保存主题 ${theme.name}`}>
          <Save className="w-3 h-3 mr-1" aria-hidden="true" /> {saving ? "保存中..." : "保存到主题库"}
        </Button>
      </div>
    </div>
  );
}
