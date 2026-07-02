import { useEffect, useState } from "react";
import { Save, Plug, Check, AlertCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { useSettingsStore } from "@/stores/settingsStore";
import { settingsService, systemService } from "@/services";
import { CONTEXT_DEFAULTS } from "@/lib/constants";

/** 通用设置：API Key / Base URL / Model + 上下文压缩配置 */
export function GeneralSettings() {
  const s = useSettingsStore();
  const [localKey, setLocalKey] = useState(s.apiKey);
  const [localUrl, setLocalUrl] = useState(s.apiBaseUrl);
  const [localModel, setLocalModel] = useState(s.defaultModel);
  const [ctxEnabled, setCtxEnabled] = useState(CONTEXT_DEFAULTS.enabled);
  const [ctxWindow, setCtxWindow] = useState(CONTEXT_DEFAULTS.recentWindow);
  const [ctxInterval, setCtxInterval] = useState(CONTEXT_DEFAULTS.summaryInterval);
  const [ctxMaxTokens, setCtxMaxTokens] = useState(CONTEXT_DEFAULTS.maxTokens);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    settingsService.get("context_enabled").then((v) => {
      if (v) setCtxEnabled(v === "true");
    });
    settingsService.get("context_recent_window").then((v) => {
      if (v) setCtxWindow(v);
    });
    settingsService.get("context_summary_interval").then((v) => {
      if (v) setCtxInterval(v);
    });
    settingsService.get("context_max_tokens").then((v) => {
      if (v) setCtxMaxTokens(v);
    });
  }, []);

  const saveAll = async () => {
    await s.setApiKey(localKey);
    await s.setApiBaseUrl(localUrl);
    await s.setDefaultModel(localModel);
    await settingsService.set("context_enabled", ctxEnabled ? "true" : "false");
    await settingsService.set("context_recent_window", ctxWindow);
    await settingsService.set("context_summary_interval", ctxInterval);
    await settingsService.set("context_max_tokens", ctxMaxTokens);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const testApi = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const res = await systemService.testChat(localKey);
      setTestResult({ ok: true, msg: `API OK: "${res}"` });
    } catch (e) {
      setTestResult({ ok: false, msg: `API FAIL: ${e}` });
    }
    setTesting(false);
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>API Key</Label>
        <Input
          type="password"
          value={localKey}
          onChange={(e) => setLocalKey(e.target.value)}
        />
      </div>
      <div className="space-y-1">
        <Label>API Base URL</Label>
        <Input value={localUrl} onChange={(e) => setLocalUrl(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>Default Model</Label>
        <Input value={localModel} onChange={(e) => setLocalModel(e.target.value)} />
      </div>

      <Separator />

      <div className="text-xs font-medium text-muted-foreground">
        Context Window（长对话压缩）
      </div>
      <div className="flex items-center gap-2 text-xs">
        <Switch checked={ctxEnabled} onCheckedChange={setCtxEnabled} id="ctx-enabled" />
        <Label htmlFor="ctx-enabled" className="cursor-pointer">
          启用上下文压缩
        </Label>
      </div>
      {ctxEnabled && (
        <div className="grid grid-cols-3 gap-2 text-xs">
          <div className="space-y-1">
            <Label className="text-muted-foreground">最近消息数</Label>
            <Input
              type="number"
              value={ctxWindow}
              onChange={(e) => setCtxWindow(e.target.value)}
              min={5}
              max={200}
              className="h-7"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-muted-foreground">摘要间隔</Label>
            <Input
              type="number"
              value={ctxInterval}
              onChange={(e) => setCtxInterval(e.target.value)}
              min={5}
              max={50}
              className="h-7"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-muted-foreground">最大 Token</Label>
            <Input
              type="number"
              value={ctxMaxTokens}
              onChange={(e) => setCtxMaxTokens(e.target.value)}
              min={4000}
              max={128000}
              className="h-7"
            />
          </div>
        </div>
      )}

      <Button onClick={saveAll} className="w-full">
        <Save className="w-3.5 h-3.5 mr-1" />
        {saved ? "已保存" : "保存设置"}
      </Button>
      <Button onClick={testApi} disabled={testing} variant="secondary" className="w-full">
        <Plug className="w-3.5 h-3.5 mr-1" />
        {testing ? "测试中..." : "测试 API 连接"}
      </Button>
      {testResult && (
        <div
          className={`p-2 rounded text-xs flex items-start gap-1.5 ${
            testResult.ok
              ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
              : "bg-destructive/10 text-destructive"
          }`}
        >
          {testResult.ok ? (
            <Check className="w-3.5 h-3.5 mt-0.5 shrink-0" />
          ) : (
            <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
          )}
          <span className="break-all">{testResult.msg}</span>
        </div>
      )}
    </div>
  );
}
