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

  /** 读取 Agent 工具权限级别：strict / elevated / full */
  getPermissionLevel: (id: string): Promise<string> =>
    invoke<string>("get_agent_permission_level", { agentId: id }),

  /** 设置 Agent 工具权限级别 */
  setPermissionLevel: (id: string, level: string): Promise<void> =>
    invoke<void>("set_agent_permission_level", { agentId: id, level }),

  /** 列出 Agent 已信任的工具（elevated 模式下积累） */
  listTrustedTools: (id: string): Promise<string[]> =>
    invoke<string[]>("list_trusted_tools", { agentId: id }),

  /** 收回某工具的信任 */
  revokeTrust: (id: string, toolName: string): Promise<void> =>
    invoke<void>("revoke_trust", { agentId: id, toolName }),
};
