import { useEffect, useState } from "react";
import { Save, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { agentService } from "@/services/agent.service";
import { useAgentStore } from "@/stores/agentStore";
import type { Agent } from "@/types";

interface AgentEditorPanelProps {
  agent: Agent;
}

/** Agent 编辑器：名称 / 描述 / system prompt */
export function AgentEditorPanel({ agent }: AgentEditorPanelProps) {
  const [name, setName] = useState(agent.name);
  const [desc, setDesc] = useState(agent.description);
  const [prompt, setPrompt] = useState(agent.system_prompt);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");

  useEffect(() => {
    setName(agent.name);
    setDesc(agent.description);
    setPrompt(agent.system_prompt);
    setErr("");
  }, [agent.id]);

  const handleSave = async () => {
    setSaving(true);
    setErr("");
    try {
      // 注意：兼容 Tauri v2 的 camelCase/snake_case 转换，同时传两种命名
      await agentService.update(agent.id, {
        name,
        description: desc,
        system_prompt: prompt,
        systemPrompt: prompt,
      } as any);
      await useAgentStore.getState().loadAgents();
      const found = useAgentStore.getState().agents.find((a) => a.id === agent.id);
      if (found) useAgentStore.getState().selectAgent(found);
    } catch (e) {
      setErr(String(e));
    }
    setSaving(false);
  };

  return (
    <div className="p-3 space-y-3 text-xs">
      <div className="space-y-1">
        <Label className="text-muted-foreground">名称</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} className="h-7 text-xs" />
      </div>
      <div className="space-y-1">
        <Label className="text-muted-foreground">描述</Label>
        <Input value={desc} onChange={(e) => setDesc(e.target.value)} className="h-7 text-xs" />
      </div>
      <div className="space-y-1">
        <Label className="text-muted-foreground">System Prompt</Label>
        <Textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          rows={8}
          className="font-mono text-[11px]"
        />
      </div>
      <Button onClick={handleSave} disabled={saving} size="sm" className="w-full h-7">
        {saving ? (
          "保存中..."
        ) : (
          <>
            <Save className="w-3 h-3 mr-1" />
            保存
          </>
        )}
      </Button>
      {err && <div className="text-destructive text-[10px]">{err}</div>}
      {!err && (
        <div className="text-emerald-500 text-[10px] text-center flex items-center justify-center gap-1">
          <Check className="w-2.5 h-2.5" />
          提示：用 {`{key}`} 注入 Agents/*.txt 内容
        </div>
      )}
    </div>
  );
}
