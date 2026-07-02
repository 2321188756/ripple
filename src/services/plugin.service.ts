import { invoke } from "./invoke";
import type { PluginManifest } from "@/types";

export const pluginService = {
  list: (): Promise<PluginManifest[]> =>
    invoke<PluginManifest[]>("list_plugins"),

  toggle: (name: string, enabled: boolean): Promise<void> =>
    invoke<void>("toggle_plugin", { name, enabled }),

  getConfig: (name: string): Promise<Record<string, unknown>> =>
    invoke<Record<string, unknown>>("get_plugin_config", { name }),

  setConfig: (
    name: string,
    config: Record<string, unknown>,
  ): Promise<void> =>
    invoke<void>("set_plugin_config", { name, config }),

  executeTool: (
    toolName: string,
    args: Record<string, unknown>,
  ): Promise<string> =>
    invoke<string>("execute_plugin_tool", { toolName, args }),
};
