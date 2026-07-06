import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useAgentStore } from "@/stores/agentStore";
import { cn } from "@/lib/utils";
import type { Agent } from "@/types";

const EMOJIS = ["🤖", "✨", "🧠", "⚡", "🎨", "🔧", "📝", "🎯", "💡", "🛡️"];

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

  useEffect(() => { loadAgents(); }, [loadAgents]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    const id = await createAgent(newName.trim());
    const agent = useAgentStore.getState().agents.find((a) => a.id === id);
    if (agent) onSelect(agent);
    setNewName("");
    setExpanded(false);
    setPickedIcon(0);
  };

  return (
    <div className="flex flex-col h-full">
      {/* 新建区域 */}
      <div className="p-3 border-b border-border space-y-2">
        {!expanded ? (
          <Button variant="outline" size="sm" onClick={() => setExpanded(true)}
            className="w-full h-9 text-sm border-dashed border-2 text-muted-foreground hover:text-foreground hover:border-primary/40">
            <Plus className="w-4 h-4 mr-2" />新建 Agent
          </Button>
        ) : (
          <div className="space-y-2 p-2 bg-accent/30 rounded-lg border border-border animate-fade-in">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center shrink-0 bg-primary/10 text-lg">
                {EMOJIS[pickedIcon]}
              </div>
              <div className="flex-1 space-y-1.5">
                <Input autoFocus value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleCreate(); if (e.key === "Escape") setExpanded(false); }}
                  placeholder="Agent 名称..." className="h-8 text-sm" />
                <div className="flex gap-1.5">
                  <Button onClick={handleCreate} disabled={!newName.trim()} size="sm" className="h-7 text-xs flex-1">创建</Button>
                  <Button variant="ghost" size="sm" onClick={() => setExpanded(false)} className="h-7 text-xs">取消</Button>
                </div>
              </div>
            </div>
            <div className="flex gap-1 flex-wrap">
              {EMOJIS.map((e, i) => (
                <button key={e} onClick={() => setPickedIcon(i)}
                  className={cn("w-7 h-7 rounded text-sm flex items-center justify-center transition-all", pickedIcon === i ? "bg-primary/20 ring-1 ring-primary" : "hover:bg-muted")}>
                  {e}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Agent 列表 */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-1">
          {agents.length === 0 && !expanded && (
            <div className="text-center py-8 text-muted-foreground">
              <p className="text-xs">还没有 Agent</p>
              <p className="text-[10px] mt-0.5">点击上方按钮创建</p>
            </div>
          )}
          {agents.map((a) => (
            <button key={a.id} onClick={() => onSelect(a)}
              className={cn(
                "w-full flex items-center gap-3 px-3 py-2 text-left transition-all duration-150 border-b border-border/10",
                selectedAgent?.id === a.id
                  ? "bg-primary/5"
                  : "hover:bg-accent/40",
              )}>
              <div className="h-9 w-9 shrink-0 rounded-full flex items-center justify-center text-base overflow-hidden"
                style={{
                  backgroundColor: a.icon_color || "#6366f1",
                  border: `${a.border_width || 2}px solid ${a.border_color || "#6366f1"}`,
                }}>
                {a.icon?.startsWith("data:image") ? (
                  <img src={a.icon} alt="" className="w-full h-full object-cover" />
                ) : (
                  a.icon || "🤖"
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium truncate" style={{ color: a.name_color || undefined }}>{a.name}</div>
              </div>
              {selectedAgent?.id === a.id && (
                <div className="w-2 h-2 rounded-full bg-primary shrink-0" />
              )}
            </button>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
