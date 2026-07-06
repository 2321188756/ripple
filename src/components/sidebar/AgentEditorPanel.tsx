import { useEffect, useRef, useState } from "react";
import { Save, Check, Upload, RotateCcw, Shield } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { agentService } from "@/services/agent.service";
import { useAgentStore } from "@/stores/agentStore";
import { cn } from "@/lib/utils";
import type { Agent } from "@/types";

interface AgentEditorPanelProps {
  agent: Agent;
}

export function AgentEditorPanel({ agent }: AgentEditorPanelProps) {
  const [name, setName] = useState(agent.name);
  const [desc, setDesc] = useState(agent.description);
  const [prompt, setPrompt] = useState(agent.system_prompt);
  const [icon, setIcon] = useState(agent.icon || "🤖");
  const [iconBg, setIconBg] = useState(agent.icon_color || "#6366f1");
  const [borderColor, setBorderColor] = useState(agent.border_color || "#6366f1");
  const [borderWidth, setBorderWidth] = useState(agent.border_width || 2);
  const [nameColor, setNameColor] = useState(agent.name_color || "#1e293b");
  const [temperature, setTemperature] = useState(agent.temperature ?? 0.7);
  const [maxTokens, setMaxTokens] = useState(agent.max_tokens ?? 4096);
  const [topP, setTopP] = useState(agent.top_p ?? 1.0);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");
  const [saved, setSaved] = useState(false);
  const [permissionLevel, setPermissionLevel] = useState("strict");
  const [trustedTools, setTrustedTools] = useState<string[]>([]);
  const fileRef = useRef<HTMLInputElement>(null);
  const [customAvatar, setCustomAvatar] = useState<string | null>(
    agent.icon?.startsWith("data:image") ? agent.icon : null
  );

  useEffect(() => {
    setName(agent.name); setDesc(agent.description); setPrompt(agent.system_prompt);
    setIcon(agent.icon || "🤖"); setIconBg(agent.icon_color || "#6366f1");
    setBorderColor(agent.border_color || "#6366f1"); setBorderWidth(agent.border_width || 3);
    setNameColor(agent.name_color || "#1e293b");
    setTemperature(agent.temperature ?? 0.7); setMaxTokens(agent.max_tokens ?? 4096); setTopP(agent.top_p ?? 1.0);
    setCustomAvatar(agent.icon?.startsWith("data:image") ? agent.icon : null);
    setErr(""); setSaved(false);
  }, [agent.id]);

  // 加载工具权限级别 + 已信任工具
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [lvl, tools] = await Promise.all([
          agentService.getPermissionLevel(agent.id),
          agentService.listTrustedTools(agent.id),
        ]);
        if (cancelled) return;
        setPermissionLevel(lvl || "strict");
        setTrustedTools(tools);
      } catch (e) { /* 默认 strict，无信任 */ }
    })();
    return () => { cancelled = true; };
  }, [agent.id]);

  const changeLevel = async (level: string) => {
    setPermissionLevel(level);
    try { await agentService.setPermissionLevel(agent.id, level); }
    catch (e) { setErr(String(e)); }
  };

  const revokeTool = async (toolName: string) => {
    try {
      await agentService.revokeTrust(agent.id, toolName);
      setTrustedTools((prev) => prev.filter((t) => t !== toolName));
    } catch (e) { setErr(String(e)); }
  };

  const handleAvatarUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !file.type.startsWith("image/")) return;
    const reader = new FileReader();
    reader.onload = () => {
      setCustomAvatar(reader.result as string);
      setIcon(reader.result as string);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  };

  const resetAvatar = () => { setCustomAvatar(null); setIcon("🤖"); };

  const handleSave = async () => {
    setSaving(true); setErr("");
    try {
      await agentService.update(agent.id, {
        name, description: desc, system_prompt: prompt, systemPrompt: prompt,
        icon, iconColor: iconBg, borderColor, borderWidth, nameColor,
        temperature, maxTokens, topP,
      } as any);
      await useAgentStore.getState().loadAgents();
      const found = useAgentStore.getState().agents.find((a) => a.id === agent.id);
      if (found) useAgentStore.getState().selectAgent(found);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) { setErr(String(e)); }
    setSaving(false);
  };

  return (
    <div className="p-3 space-y-3 text-xs">
      <input ref={fileRef} type="file" accept="image/*" className="hidden" onChange={handleAvatarUpload} />

      {/* 基本信息 */}
      <div className="rounded-md border border-border bg-card p-3 space-y-2.5">
        <div className="text-xs font-medium text-foreground">基本信息</div>

        {/* 头像 + 名称 */}
        <div className="flex items-center gap-3">
          <div className="relative shrink-0">
            <div className="w-12 h-12 rounded-full flex items-center justify-center text-lg select-none overflow-hidden"
              style={{ backgroundColor: iconBg, border: `${Math.max(2, borderWidth)}px solid ${borderColor}` }}>
              {customAvatar ? <img src={customAvatar} alt="" className="w-full h-full object-cover" /> : <span>{icon}</span>}
            </div>
            <button onClick={() => fileRef.current?.click()}
              className="absolute -bottom-0.5 -right-0.5 w-4.5 h-4.5 rounded-full bg-background border border-border flex items-center justify-center text-muted-foreground hover:text-foreground shadow-xs">
              <Upload className="w-2.5 h-2.5" />
            </button>
            {customAvatar && (
              <button onClick={resetAvatar}
                className="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 rounded-full bg-background border border-border flex items-center justify-center text-muted-foreground hover:text-foreground">
                <RotateCcw className="w-2 h-2" />
              </button>
            )}
          </div>
          <div className="flex-1 min-w-0 space-y-1.5">
            <Input value={name} onChange={(e) => setName(e.target.value)}
              className="h-7 text-sm font-semibold" style={{ color: nameColor }} />
            <Input value={desc} onChange={(e) => setDesc(e.target.value)}
              placeholder="描述..." className="h-6 text-xs" />
          </div>
        </div>

        {/* 颜色 + 边框 */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-[10px] text-muted-foreground">字体颜色</span>
            <input type="color" value={nameColor} onChange={(e) => setNameColor(e.target.value)}
              className="w-7 h-7 rounded cursor-pointer border border-border p-0 bg-transparent" />
          </div>
          <div className="flex items-center justify-between">
            <span className="text-[10px] text-muted-foreground">边框颜色</span>
            <input type="color" value={borderColor} onChange={(e) => setBorderColor(e.target.value)}
              className="w-7 h-7 rounded cursor-pointer border border-border p-0 bg-transparent" />
          </div>
          <div className="flex items-center justify-between">
            <span className="text-[10px] text-muted-foreground">边框粗细</span>
            <div className="flex items-center gap-1">
              <input type="range" min={1} max={2} step={0.5} value={borderWidth}
                onChange={(e) => setBorderWidth(Number(e.target.value))}
                className="w-20 h-1" />
              <span className="text-[10px] text-muted-foreground w-3 text-right">{borderWidth}px</span>
            </div>
          </div>
        </div>
      </div>

      {/* 模型参数 */}
      <div className="rounded-md border border-border bg-card p-3 space-y-2.5">
        <div className="text-xs font-medium text-foreground">模型参数</div>
        <div className="grid grid-cols-3 gap-2">
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">Temperature</Label>
            <input type="number" min={0} max={2} step={0.1} value={temperature}
              onChange={(e) => setTemperature(Number(e.target.value))}
              className="w-full h-7 text-xs text-center border border-border rounded bg-background font-mono" />
          </div>
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">Max Tokens</Label>
            <input type="number" min={256} max={128000} step={256} value={maxTokens}
              onChange={(e) => setMaxTokens(Number(e.target.value))}
              className="w-full h-7 text-xs text-center border border-border rounded bg-background font-mono" />
          </div>
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">Top-P</Label>
            <input type="number" min={0} max={1} step={0.05} value={topP}
              onChange={(e) => setTopP(Number(e.target.value))}
              className="w-full h-7 text-xs text-center border border-border rounded bg-background font-mono" />
          </div>
        </div>
      </div>

      {/* 工具权限 */}
      <div className="rounded-md border border-border bg-card p-3 space-y-2.5">
        <div className="flex items-center gap-1.5 text-xs font-medium text-foreground">
          <Shield className="w-3 h-3" />
          工具权限
        </div>
        <div className="space-y-1">
          <Label className="text-[10px] text-muted-foreground">权限级别（控制插件工具审批）</Label>
          <div className="flex gap-1">
            {([
              { v: "strict", label: "严格", hint: "每次审批" },
              { v: "elevated", label: "标准", hint: "可信任积累" },
              { v: "full", label: "完全", hint: "全放行" },
            ] as const).map((opt) => (
              <button key={opt.v} type="button" onClick={() => changeLevel(opt.v)}
                className={cn("flex-1 px-2 py-1.5 rounded text-[10px] border transition-colors",
                  permissionLevel === opt.v
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-border text-muted-foreground hover:bg-accent")}>
                <div className="font-medium">{opt.label}</div>
                <div className="text-[9px] opacity-70">{opt.hint}</div>
              </button>
            ))}
          </div>
        </div>
        {permissionLevel === "elevated" && (
          <div className="space-y-1">
            <Label className="text-[10px] text-muted-foreground">已信任工具（点击收回）</Label>
            {trustedTools.length === 0 ? (
              <p className="text-[10px] text-muted-foreground italic">无。审批时勾「信任此工具」可积累。</p>
            ) : (
              <div className="flex flex-wrap gap-1">
                {trustedTools.map((t) => (
                  <button key={t} type="button" onClick={() => revokeTool(t)}
                    className="px-1.5 py-0.5 rounded text-[10px] font-mono border border-border bg-muted/30 hover:bg-destructive/10 hover:border-destructive/30 text-muted-foreground hover:text-destructive">
                    {t} ✕
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        {permissionLevel === "full" && (
          <p className="text-[10px] text-amber-600 dark:text-amber-400">
            ⚠ 该 Agent 调用所有插件工具（含 shell-exec / code-runner）将自动执行，无需审批。
          </p>
        )}
      </div>

      {/* 系统提示词 */}
      <div className="rounded-sm border border-border bg-card p-3 space-y-1.5">
        <Label className="text-xs font-medium text-foreground">System Prompt</Label>
        <Textarea value={prompt} onChange={(e) => setPrompt(e.target.value)}
          rows={3} className="font-mono text-[11px] min-h-[60px]" />
        <details className="text-[10px] text-muted-foreground">
          <summary className="cursor-pointer hover:text-foreground">记忆占位符说明</summary>
          <ul className="mt-1 space-y-0.5 list-disc pl-3">
            <li><code>{'{MEMORIES}'}</code> — 注入最近 10 条记忆</li>
            <li><code>{'<<MEMORIES>>'}</code> — 语义检索（推荐）</li>
            <li><code>{'<MEMORIES>'}</code> — 关键词检索</li>
          </ul>
        </details>
      </div>

      <Button onClick={handleSave} disabled={saving} size="sm" className="w-full h-8 text-xs">
        {saving ? "保存中..." : saved ? <><Check className="w-3.5 h-3.5 mr-1" />已保存</> : <><Save className="w-3.5 h-3.5 mr-1" />保存</>}
      </Button>
      {err && <div className="text-destructive text-[10px]">{err}</div>}
    </div>
  );
}
