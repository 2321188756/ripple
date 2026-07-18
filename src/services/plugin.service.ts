import { invoke } from "./invoke";

export interface PluginTool {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

export type PluginConfigValue = string | number | boolean;

export interface PluginConfigProperty {
  type?: "string" | "number" | "integer" | "boolean";
  description?: string;
  default?: PluginConfigValue;
  enum?: PluginConfigValue[];
  format?: string;
  sensitive?: boolean;
  writeOnly?: boolean;
}

export interface PluginManifest {
  name: string;
  version: string;
  mode?: "tool" | "transform" | "daemon";
  runtime?: "rhai" | "node" | "python" | "py" | "shell" | "bash";
  description?: string;
  author?: string;
  permissions?: string[];
  tools?: PluginTool[];
  enabled: boolean;
  config_schema?: {
    type?: "object";
    properties?: Record<string, PluginConfigProperty>;
    required?: string[];
  };
}

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
