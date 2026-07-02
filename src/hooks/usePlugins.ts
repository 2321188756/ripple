import { useEffect, useState } from "react";
import { pluginService } from "@/services/plugin.service";
import type { PluginManifest } from "@/types";

/**
 * 插件列表与配置编辑 hook。
 */
export function usePlugins() {
  const [plugins, setPlugins] = useState<PluginManifest[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [editVals, setEditVals] = useState<Record<string, string>>({});

  const load = async () => {
    try {
      setPlugins(await pluginService.list());
    } catch {
      setPlugins([]);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const openConfig = async (name: string) => {
    const cfg = await pluginService.getConfig(name);
    const flat: Record<string, string> = {};
    for (const k of Object.keys(cfg)) flat[k] = String(cfg[k]);
    setEditVals(flat);
    setEditing(name);
  };

  const saveConfig = async (name: string) => {
    const plugin = plugins.find((p) => p.name === name);
    const schema = plugin?.config_schema;
    const cfg: Record<string, unknown> = {};
    if (schema?.properties) {
      for (const key of Object.keys(schema.properties)) {
        cfg[key] = editVals[key] || "";
      }
    } else {
      Object.assign(cfg, editVals);
    }
    await pluginService.setConfig(name, cfg);
    setEditing(null);
  };

  return {
    plugins,
    editing,
    editVals,
    setEditVals,
    openConfig,
    saveConfig,
    cancelEdit: () => setEditing(null),
    reload: load,
  };
}
