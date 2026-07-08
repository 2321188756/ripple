import { useEffect, useState } from "react";
import { Save, Plug, Check, AlertCircle, Eye, EyeOff, Upload } from "lucide-react";
import { emit } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useSettingsStore } from "@/stores/settingsStore";
import { settingsService, systemService, exportService } from "@/services";
import { CONTEXT_DEFAULTS } from "@/lib/constants";

export function GeneralSettings() {
  const s = useSettingsStore();
  const [localKey, setLocalKey] = useState(s.apiKey);
  const [localUrl, setLocalUrl] = useState(s.apiBaseUrl);
  const [localModel, setLocalModel] = useState(s.defaultModel);
  const [localLlmModel, setLocalLlmModel] = useState(s.llmModel);
  const [showKey, setShowKey] = useState(false);
  const [ctxEnabled, setCtxEnabled] = useState(CONTEXT_DEFAULTS.enabled);
  const [ctxWindow, setCtxWindow] = useState(CONTEXT_DEFAULTS.recentWindow);
  const [ctxInterval, setCtxInterval] = useState(CONTEXT_DEFAULTS.summaryInterval);
  const [ctxMaxTokens, setCtxMaxTokens] = useState(CONTEXT_DEFAULTS.maxTokens);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [saved, setSaved] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [importMsg, setImportMsg] = useState<string | null>(null);
  const [debugLogging, setDebugLogging] = useState(false);

  useEffect(() => {
    settingsService.get("context_enabled").then((v) => { if (v) setCtxEnabled(v === "true"); });
    settingsService.get("context_recent_window").then((v) => { if (v) setCtxWindow(v); });
    settingsService.get("context_summary_interval").then((v) => { if (v) setCtxInterval(v); });
    settingsService.get("context_max_tokens").then((v) => { if (v) setCtxMaxTokens(v); });
    settingsService.getDebugLogging().then(setDebugLogging).catch(() => {});
  }, []);

  const markDirty = () => setDirty(true);

  const toggleDebug = async (enabled: boolean) => {
    setDebugLogging(enabled);
    await settingsService.setDebugLogging(enabled);
  };

  const handleImport = async () => {
    setImportMsg(null);
    try {
      const file = await open({
        multiple: false,
        filters: [{ name: "Conversation JSON", extensions: ["json"] }],
      });
      if (!file) return;
      const text = await readTextFile(file as string);
      const newId = await exportService.importConversation(text);
      setImportMsg(`已导入对话：${newId.slice(0, 8)}`);
      // 通知主窗口刷新会话列表
      void emit("ripple:conversations-changed");
    } catch (e) {
      setImportMsg(`导入失败：${e}`);
    }
  };

  const saveAll = async () => {
    await s.setApiKey(localKey);
    await s.setApiBaseUrl(localUrl);
    await s.setDefaultModel(localModel);
    await s.setLlmModel(localLlmModel);
    await settingsService.set("context_enabled", ctxEnabled ? "true" : "false");
    await settingsService.set("context_recent_window", ctxWindow);
    await settingsService.set("context_summary_interval", ctxInterval);
    await settingsService.set("context_max_tokens", ctxMaxTokens);
    setSaved(true); setDirty(false);
    setTimeout(() => setSaved(false), 2000);
  };

  const testApi = async () => {
    setTesting(true); setTestResult(null);
    try {
      const res = await systemService.testChat(localKey);
      setTestResult({ ok: true, msg: `OK: ${res}` });
    } catch (e) { setTestResult({ ok: false, msg: `失败: ${e}` }); }
    setTesting(false);
  };

  return (
    <div className="space-y-4">
      {/* API 连接 */}
      <div className="rounded-lg border border-border bg-card p-4 space-y-3">
        <div className="text-sm font-medium text-foreground">API 连接</div>

        <div className="space-y-1">
          <Label className="text-xs text-muted-foreground">API Key</Label>
          <div className="relative">
            <Input type={showKey ? "text" : "password"} value={localKey}
              onChange={(e) => { setLocalKey(e.target.value); markDirty(); }}
              className="pr-9 text-xs font-mono" />
            <button type="button" onClick={() => setShowKey(!showKey)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground">
              {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
            </button>
          </div>
        </div>

        <div className="space-y-1">
          <Label className="text-xs text-muted-foreground">API Base URL</Label>
          <Input value={localUrl} onChange={(e) => { setLocalUrl(e.target.value); markDirty(); }}
            className="text-xs font-mono" />
        </div>

        <div className="space-y-1">
          <Label className="text-xs text-muted-foreground">默认模型</Label>
          <Input value={localModel} onChange={(e) => { setLocalModel(e.target.value); markDirty(); }}
            className="text-xs font-mono" />
          <p className="text-[10px] text-muted-foreground">新对话的默认聊天模型（conversation.model_id 为 default 时回退到此）</p>
        </div>

        <div className="space-y-1">
          <Label className="text-xs text-muted-foreground">记忆 LLM 模型</Label>
          <Input value={localLlmModel} onChange={(e) => { setLocalLlmModel(e.target.value); markDirty(); }}
            className="text-xs font-mono" />
          <p className="text-[10px] text-muted-foreground">记忆标签生成 & AIMemo 总结用（高频后台任务，可用便宜模型；留空回退 deepseek-v4-flash）</p>
        </div>
      </div>

      {/* 上下文压缩 */}
      <div className="rounded-lg border border-border bg-card p-4 space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Switch checked={ctxEnabled} onCheckedChange={(v) => { setCtxEnabled(v); markDirty(); }} id="ctx" />
            <Label htmlFor="ctx" className="text-sm font-medium cursor-pointer">上下文压缩</Label>
          </div>
        </div>
        {ctxEnabled && (
          <div className="grid grid-cols-3 gap-2 text-xs">
            <div className="space-y-1">
              <Label className="text-muted-foreground">最近消息数</Label>
              <Input type="number" value={ctxWindow}
                onChange={(e) => { setCtxWindow(e.target.value); markDirty(); }}
                min={5} max={200} className="h-7" />
            </div>
            <div className="space-y-1">
              <Label className="text-muted-foreground">摘要间隔</Label>
              <Input type="number" value={ctxInterval}
                onChange={(e) => { setCtxInterval(e.target.value); markDirty(); }}
                min={5} max={50} className="h-7" />
            </div>
            <div className="space-y-1">
              <Label className="text-muted-foreground">最大 Token</Label>
              <Input type="number" value={ctxMaxTokens}
                onChange={(e) => { setCtxMaxTokens(e.target.value); markDirty(); }}
                min={4000} max={128000} className="h-7" />
            </div>
          </div>
        )}
      </div>

      {/* 测试结果 */}
      {testResult && (
        <div className={`p-3 rounded-lg text-xs flex items-start gap-2 ${
          testResult.ok ? "bg-success/10 text-success border border-success/20"
            : "bg-destructive/10 text-destructive border border-destructive/20"
        }`}>
          {testResult.ok ? <Check className="w-3.5 h-3.5 mt-0.5 shrink-0" /> : <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />}
          <span className="break-all font-mono text-[11px]">{testResult.msg}</span>
        </div>
      )}

      {/* 数据管理：对话导入 + 调试日志 */}
      <div className="rounded-lg border border-border bg-card p-4 space-y-3">
        <div className="text-sm font-medium text-foreground">数据管理</div>
        <div className="flex items-center gap-2">
          <Button onClick={handleImport} variant="outline" size="sm" className="h-8 text-xs">
            <Upload className="w-3.5 h-3.5 mr-1.5" />
            导入对话（JSON）
          </Button>
          {importMsg && (
            <span className="text-xs text-muted-foreground break-all">{importMsg}</span>
          )}
        </div>
        <p className="text-[11px] text-muted-foreground">
          从 JSON 备份文件导入对话（与 ChatHeader 的「导出 JSON」配套）。导入后会出现在主窗口会话列表。
        </p>
        <div className="flex items-center justify-between pt-2 border-t border-border">
          <div className="space-y-0.5">
            <Label className="text-xs text-foreground">调试日志</Label>
            <p className="text-[11px] text-muted-foreground">
              开启后记录请求体、流式 chunk、工具调用等细节到 logs/（运行时切换日志级别 info↔debug）
            </p>
          </div>
          <Switch checked={debugLogging} onCheckedChange={toggleDebug} />
        </div>
      </div>

      {/* 底部操作栏 */}
      <div className="flex items-center gap-2 pt-2 border-t border-border">
        <Button onClick={saveAll} size="sm" className="h-8 text-xs px-4" disabled={!dirty}>
          <Save className="w-3.5 h-3.5 mr-1.5" />
          {saved ? "已保存" : "保存"}
        </Button>
        <Button onClick={testApi} disabled={testing} variant="outline" size="sm" className="h-8 text-xs px-4">
          <Plug className="w-3.5 h-3.5 mr-1.5" />
          {testing ? "测试中..." : "测试连接"}
        </Button>
        <div className="flex-1" />
        <span className="text-[10px] text-muted-foreground">
          {dirty ? "有未保存的更改" : saved ? "已保存" : ""}
        </span>
      </div>
    </div>
  );
}
