import { useEffect, useState } from "react";
import { Plus, Bot, Sparkles, User, Zap, Brain, Code, Wand2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { useAgentStore } from "@/stores/agentStore";
import { cn } from "@/lib/utils";
import type { Agent } from "@/types";

const AGENT_ICONS = [
  { icon: Bot, color: "bg-indigo-500" },
  { icon: Sparkles, color: "bg-violet-500" },
  { icon: Brain, color: "bg-emerald-500" },
  { icon: Code, color: "bg-amber-500" },
  { icon: Zap, color: "bg-rose-500" },
  { icon: Wand2, color: "bg-cyan-500" },
  { icon: User, color: "bg-slate-500" },
];

interface AgentListViewProps {
  selectedAgent: Agent | null;
  onSelect: (agent: Agent) => void;
}

/** 侧边栏 Agent 列表 + 新建 */
export function AgentListView({ selectedAgent, onSelect }: AgentListViewProps) {
  const { agents, createAgent, loadAgents } = useAgentStore();
  const [newName, setNewName] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [pickedIcon, setPickedIcon] = useState(0);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    const id = await createAgent(newName.trim());
    const agent = useAgentStore.getState().agents.find((a) => a.id === id);
    if (agent) {
      // 更新 icon 为选中的图标
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("update_agent", { id, icon: AGENT_ICONS[pickedIcon].icon.name });
      } catch { /* icon update is best-effort */ }
      onSelect(agent);
    }
    setNewName("");
    setExpanded(false);
    setPickedIcon(0);
  };

  const cycleIcon = () => setPickedIcon((i) => (i + 1) % AGENT_ICONS.length);
  const PickedIcon = AGENT_ICONS[pickedIcon].icon;

  return (
    <div className="flex flex-col h-full">
      {/* 新建区域 */}
      <div className="p-3 border-b border-border space-y-2">
        {!expanded ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => setExpanded(true)}
            className="w-full h-9 text-sm border-dashed border-2 text-muted-foreground hover:text-foreground hover:border-primary/40"
          >
            <Plus className="w-4 h-4 mr-2" />
            新建 Agent
          </Button>
        ) : (
          <div className="space-y-2 p-2 bg-accent/30 rounded-lg border border-border animate-fade-in">
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={cycleIcon}
                className={cn(
                  "w-10 h-10 rounded-xl flex items-center justify-center shrink-0 transition-colors cursor-pointer",
                  AGENT_ICONS[pickedIcon].color,
                )}
                title="切换图标"
              >
                <PickedIcon className="w-5 h-5 text-white" />
              </button>
              <div className="flex-1 space-y-1.5">
                <Input
                  autoFocus
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleCreate();
                    if (e.key === "Escape") setExpanded(false);
                  }}
                  placeholder="Agent 名称..."
                  className="h-8 text-sm"
                />
                <div className="flex gap-1.5">
                  <Button
                    onClick={handleCreate}
                    disabled={!newName.trim()}
                    size="sm"
                    className="h-7 text-xs flex-1"
                  >
                    创建
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setExpanded(false)}
                    className="h-7 text-xs"
                  >
                    取消
                  </Button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Agent 列表 */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-1">
          {agents.length === 0 && !expanded && (
            <div className="text-center py-8 text-muted-foreground">
              <Bot className="w-8 h-8 mx-auto mb-2 opacity-30" />
              <p className="text-xs">还没有 Agent</p>
              <p className="text-[10px] mt-0.5">点击上方按钮创建</p>
            </div>
          )}
          {agents.map((a, idx) => {
            const iconCfg = AGENT_ICONS[idx % AGENT_ICONS.length];
            const IconComp = iconCfg.icon;
            return (
              <button
                key={a.id}
                onClick={() => onSelect(a)}
                className={cn(
                  "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-all duration-150",
                  selectedAgent?.id === a.id
                    ? "bg-primary/10 border border-primary/30 shadow-sm"
                    : "border border-transparent hover:bg-accent/60 hover:border-border",
                )}
              >
                <Avatar className="h-9 w-9 shrink-0">
                  <AvatarFallback className={cn("text-white", iconCfg.color)}>
                    <IconComp className="w-4 h-4" />
                  </AvatarFallback>
                </Avatar>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">{a.name}</div>
                  <div className="text-[11px] text-muted-foreground truncate mt-0.5">
                    {a.description || a.system_prompt?.slice(0, 40) || "点击选择此 Agent"}
                  </div>
                </div>
                {selectedAgent?.id === a.id && (
                  <div className="w-2 h-2 rounded-full bg-primary shrink-0" />
                )}
              </button>
            );
          })}
        </div>
      </ScrollArea>
    </div>
  );
}