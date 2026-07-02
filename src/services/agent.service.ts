import { invoke } from "./invoke";
import type { Agent } from "@/types";

export const agentService = {
  list: (): Promise<Agent[]> => invoke<Agent[]>("list_agents"),

  create: (params: {
    name: string;
    description?: string;
    systemPrompt?: string;
  }): Promise<Agent> =>
    invoke<Agent>("create_agent", {
      name: params.name,
      description: params.description || "",
      system_prompt: params.systemPrompt || "You are a helpful assistant.",
    }),

  update: (id: string, updates: Partial<Agent>): Promise<void> =>
    invoke<void>("update_agent", { id, ...updates }),

  delete: (id: string): Promise<void> =>
    invoke<void>("delete_agent", { id }),

  get: (id: string): Promise<Agent> =>
    invoke<Agent>("get_agent", { id }),
};
