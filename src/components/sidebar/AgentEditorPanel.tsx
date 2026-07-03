import { useEffect, useRef, useState } from "react";
import { Save, Check, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Slider } from "@/components/ui/slider";
import { agentService } from "@/services/agent.service";
import { useAgentStore } from "@/stores/agentStore";
import { cn } from "@/lib/utils";
import type { Agent } from "@/types";

const EMOJIS = ["🤖", "✨", "🧠", "⚡", "🎨", "🔧", "📝", "🎯", "💡", "🛡️", "🦊", "🐱", "🌈", "🔥", "💎"];

interface AgentEditorPanelProps {
  agent: Agent;
}

/** Agent 设置编辑器——头像上传 / 颜色取色器 / 模型参数滑块 */
export function AgentEditorPanel({ agent }: AgentEditorPanelProps) {
  const [name, setName] = useState(agent.name);
  const [desc, setDesc] = useState(agent.description);
  const [prompt, setPrompt] = useState(agent.system_prompt);
  const [icon, setIcon] = useState(agent.icon || "🤖");
  const [iconBg, setIconBg] = useState(agent.icon_color || "#6366f1");
  const [borderColor, setBorderColor] = useState(agent.border_color || "#6366f1");
  const [borderWidth, setBorderWidth] = useState(agent.border_width || 3);
  const [nameColor, setNameColor] = useState(agent.name_color || "#1e293b");
  const [temperature, setTemperature] = useState(agent.temperature ?? 0.7);
  const [maxTokens, setMaxTokens] = useState(agent.max_tokens ?? 4096);
  const [topP, setTopP] = useState(agent.top_p ?? 1.0);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");
  const [saved, setSaved] = useState(false);
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

  const handleAvatarUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !file.type.startsWith("image/")) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      setCustomAvatar(dataUrl);
      setIcon(dataUrl);
    };
    reader.readAsDataURL(file);
  };

  const pickEmoji = (e: string) => {
    setIcon(e);
    setCustomAvatar(null);
  };

  const handleSave = async () => {
    setSaving(true); setErr("");
    try {
      await agentService.update(agent.id, {
        name, description: desc, system_prompt: prompt, systemPrompt: prompt,
        icon,
        iconColor: iconBg, borderColor, borderWidth, nameColor,
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
    <div className="p-4 space-y-4 text-xs">
      <input ref={fileRef} type="file" accept="image/*" className="hidden" onChange={handleAvatarUpload} />

      {/* 头像 + 名称 */}
      <div className="flex items-center gap-4">
        <div className="relative shrink-0 group">
          <div className="w-16 h-16 rounded-full flex items-center justify-center text-2xl select-none overflow-hidden"
            style={{ backgroundColor: iconBg, border: `${borderWidth}px solid ${borderColor}` }}>
            {customAvatar ? (
              <img src={customAvatar} alt="avatar" className="w-full h-full object-cover" />
            ) : (
              icon
            )}
          </div>
          <button onClick={() => fileRef.current?.click()}
            className="absolute -bottom-1 -right-1 w-6 h-6 rounded-full bg-primary text-primary-foreground flex items-center justify-center shadow-sm hover:bg-primary/90 transition-colors"
            title="上传头像">
            <Upload className="w-3 h-3" />
          </button>
        </div>
        <div className="flex-1 min-w-0 space-y-1">
          <Input value={name} onChange={(e) => setName(e.target.value)}
            className="h-8 text-sm font-semibold" style={{ color: nameColor }} />
          <Input value={desc} onChange={(e) => setDesc(e.target.value)}
            placeholder="描述..." className="h-7 text-xs" />
        </div>
      </div>

      {/* Emoji 选择 */}
      <div>
        <Label className="text-muted-foreground mb-1 block">选择 Emoji 图标</Label>
        <div className="flex flex-wrap gap-1">
          {EMOJIS.map((e) => (
            <button key={e} onClick={() => pickEmoji(e)}
              className={cn("w-7 h-7 rounded-lg text-sm flex items-center justify-center transition-all",
                icon === e && !customAvatar ? "bg-primary/20 ring-1 ring-primary" : "hover:bg-muted")}>
              {e}
            </button>
          ))}
        </div>
      </div>

      {/* 边框颜色 + 宽度 */}
      <div>
        <Label className="text-muted-foreground mb-1 block">
          边框颜色 · 宽度 ({borderWidth}px)
        </Label>
        <div className="flex items-center gap-3">
          <input type="color" value={borderColor} onChange={(e) => setBorderColor(e.target.value)}
            className="w-8 h-8 rounded cursor-pointer border border-border p-0.5 bg-transparent" />
          <input type="range" min={1} max={8} value={borderWidth}
            onChange={(e) => setBorderWidth(Number(e.target.value))}
            className="flex-1 max-w-[120px]" />
        </div>
      </div>

      {/* 名字颜色 */}
      <div>
        <Label className="text-muted-foreground mb-1 block">名字颜色</Label>
        <input type="color" value={nameColor} onChange={(e) => setNameColor(e.target.value)}
          className="w-8 h-8 rounded cursor-pointer border border-border p-0.5 bg-transparent" />
      </div>

      <Separator />

      {/* 系统提示词 */}
      <div className="space-y-1">
        <Label className="text-muted-foreground">System Prompt</Label>
        <Textarea value={prompt} onChange={(e) => setPrompt(e.target.value)}
          rows={4} className="font-mono text-[11px]" />
      </div>

      <Separator />

      {/* 模型参数 */}
      <div className="text-xs font-medium text-muted-foreground">模型参数</div>
      <div className="space-y-3">
        <div className="space-y-1">
          <div className="flex justify-between">
            <Label>Temperature</Label>
            <span className="text-muted-foreground">{temperature.toFixed(1)}</span>
          </div>
          <Slider value={[temperature]} min={0} max={2} step={0.1}
            onValueChange={([v]: number[]) => setTemperature(v)} />
        </div>
        <div className="space-y-1">
          <div className="flex justify-between">
            <Label>Max Tokens</Label>
            <span className="text-muted-foreground">{maxTokens}</span>
          </div>
          <Slider value={[maxTokens]} min={256} max={128000} step={256}
            onValueChange={([v]: number[]) => setMaxTokens(v)} />
        </div>
        <div className="space-y-1">
          <div className="flex justify-between">
            <Label>Top-P</Label>
            <span className="text-muted-foreground">{topP.toFixed(2)}</span>
          </div>
          <Slider value={[topP]} min={0} max={1} step={0.05}
            onValueChange={([v]: number[]) => setTopP(v)} />
        </div>
      </div>

      <Button onClick={handleSave} disabled={saving} size="sm" className="w-full h-8">
        {saving ? "保存中..." : saved ? <><Check className="w-3.5 h-3.5 mr-1" />已保存</> : <><Save className="w-3.5 h-3.5 mr-1" />保存</>}
      </Button>
      {err && <div className="text-destructive text-[10px]">{err}</div>}
    </div>
  );
}
