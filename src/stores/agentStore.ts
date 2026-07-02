import { create } from "zustand";
import { agentService } from "@/services/agent.service";
import type { Agent } from "@/types";
import type { SidebarTab } from "@/types/theme";

interface AgentState {
  agents: Agent[];
  selectedAgent: Agent | null;
  sidebarTab: SidebarTab;
  editing: boolean;

  loadAgents: () => Promise<void>;
  selectAgent: (agent: Agent) => void;
  createAgent: (name: string, description?: string, systemPrompt?: string) => Promise<string>;
  updateAgent: (id: string, updates: Partial<Agent>) => Promise<void>;
  deleteAgent: (id: string) => Promise<void>;
  setSidebarTab: (tab: SidebarTab) => void;
  setEditing: (v: boolean) => void;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  agents: [],
  selectedAgent: null,
  sidebarTab: "agents",
  editing: false,

  loadAgents: async () => {
    const agents = await agentService.list().catch(() => []);
    set({ agents });
  },

  selectAgent: (agent: Agent) => {
    // 只选中 Agent，不自动跳 tab。右侧自动恢复该 Agent 的上次会话。
    set({ selectedAgent: agent, editing: false });
  },

  createAgent: async (name, description?, systemPrompt?) => {
    const agent = await agentService.create({ name, description, systemPrompt });
    await get().loadAgents();
    return agent.id;
  },

  updateAgent: async (id, updates) => {
    try {
      await agentService.update(id, updates);
      await get().loadAgents();
      const selected = get().selectedAgent;
      if (selected && selected.id === id) {
        set({ selectedAgent: { ...selected, ...updates } as Agent });
      }
    } catch (e) {
      console.error("update_agent failed:", e);
      throw e;
    }
  },

  deleteAgent: async (id) => {
    await agentService.delete(id);
    const s = get();
    if (s.selectedAgent?.id === id) set({ selectedAgent: null, sidebarTab: "agents" });
    await s.loadAgents();
  },

  setSidebarTab: (tab) => set({ sidebarTab: tab }),
  setEditing: (v) => set({ editing: v }),
}));
