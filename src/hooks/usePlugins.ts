import { useCallback, useEffect, useState } from "react";
import {
  pluginService,
  type PluginConfigValue,
  type PluginManifest,
} from "@/services/plugin.service";

const MASKED_VALUE = "••••••••";

function initialValues(plugin: PluginManifest, config: Record<string, unknown>) {
  const values: Record<string, PluginConfigValue> = {};
  for (const [key, property] of Object.entries(plugin.config_schema?.properties ?? {})) {
    const value = config[key] ?? property.default ?? (property.type === "boolean" ? false : "");
    values[key] = typeof value === "boolean" || typeof value === "number" ? value : String(value);
  }
  return values;
}

export function isSensitiveConfigField(name: string, property: { format?: string; sensitive?: boolean; writeOnly?: boolean }) {
  return property.sensitive === true || property.writeOnly === true
    || property.format === "password" || property.format === "secret"
    || /password|secret|token|api_?key|credential/i.test(name);
}

export function usePlugins() {
  const [plugins, setPlugins] = useState<PluginManifest[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [editVals, setEditVals] = useState<Record<string, PluginConfigValue>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setPlugins(await pluginService.list());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const openConfig = async (name: string) => {
    setError(null);
    try {
      const plugin = plugins.find((candidate) => candidate.name === name);
      if (!plugin) throw new Error(`未找到插件：${name}`);
      setEditVals(initialValues(plugin, await pluginService.getConfig(name)));
      setEditing(name);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const saveConfig = async (name: string) => {
    setSaving(true);
    setError(null);
    try {
      const plugin = plugins.find((candidate) => candidate.name === name);
      if (!plugin) throw new Error(`未找到插件：${name}`);
      const config: Record<string, unknown> = {};
      for (const [key, property] of Object.entries(plugin.config_schema?.properties ?? {})) {
        const value = editVals[key];
        if (property.type === "number" || property.type === "integer") {
          const parsed = Number(value);
          if (!Number.isFinite(parsed) || (property.type === "integer" && !Number.isInteger(parsed))) {
            throw new Error(`${property.description || key} 必须是${property.type === "integer" ? "整数" : "数字"}`);
          }
          config[key] = parsed;
        } else if (property.type === "boolean") {
          config[key] = Boolean(value);
        } else {
          config[key] = value ?? "";
        }
      }
      await pluginService.setConfig(name, config);
      setEditing(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };

  const toggle = async (name: string, enabled: boolean) => {
    setError(null);
    setPlugins((current) => current.map((p) => p.name === name ? { ...p, enabled } : p));
    try {
      await pluginService.toggle(name, enabled);
    } catch (cause) {
      setPlugins((current) => current.map((p) => p.name === name ? { ...p, enabled: !enabled } : p));
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  return { plugins, editing, editVals, setEditVals, loading, saving, error, openConfig,
    saveConfig, toggle, cancelEdit: () => setEditing(null), reload: load, maskedValue: MASKED_VALUE };
}
