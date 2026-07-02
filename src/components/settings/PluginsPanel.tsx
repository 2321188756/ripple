import { Wrench, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { usePlugins } from "@/hooks/usePlugins";

/** 插件管理面板 */
export function PluginsPanel() {
  const { plugins, editing, editVals, setEditVals, openConfig, saveConfig, cancelEdit } =
    usePlugins();

  return (
    <div className="space-y-2 text-xs">
      <p className="text-muted-foreground">
        插件从 <code className="text-foreground/80">plugins/</code> 目录加载。
      </p>
      {plugins.map((p) => (
        <div key={p.name} className="border border-border rounded-lg p-3">
          <div className="flex justify-between items-start">
            <div className="flex-1">
              <span className="font-medium text-sm">{p.name}</span>
              <span className="text-muted-foreground ml-2">v{p.version}</span>
              <Badge
                variant={
                  p.mode === "daemon"
                    ? "secondary"
                    : p.mode === "transform"
                      ? "warning"
                      : "success"
                }
                className="ml-2"
              >
                {p.mode || "tool"}
              </Badge>
              <Badge variant="outline" className="ml-1">
                {p.runtime}
              </Badge>
            </div>
            {p.config_schema && editing !== p.name && (
              <Button
                variant="ghost"
                size="sm"
                className="h-6 text-[10px] text-primary"
                onClick={() => openConfig(p.name)}
              >
                <Settings2 className="w-3 h-3 mr-1" />
                配置
              </Button>
            )}
          </div>
          {p.description && <p className="text-muted-foreground mt-1">{p.description}</p>}
          {p.tools && p.tools.length > 0 && (
            <div className="mt-1 space-y-0.5">
              {p.tools.map((t) => (
                <div key={t.name} className="text-muted-foreground text-[10px] flex items-center gap-1">
                  <Wrench className="w-2.5 h-2.5" />
                  {t.name}: {t.description}
                </div>
              ))}
            </div>
          )}
          {/* 配置编辑 */}
          {editing === p.name && p.config_schema?.properties && (
            <div className="mt-2 space-y-2 border-t border-border pt-2">
              {Object.entries(p.config_schema.properties).map(([key, prop]) => (
                <div key={key} className="space-y-0.5">
                  <Label className="text-muted-foreground text-[10px]">
                    {prop.description || key}
                  </Label>
                  <Input
                    value={editVals[key] || ""}
                    onChange={(e) => setEditVals((v) => ({ ...v, [key]: e.target.value }))}
                    className="h-7 text-xs"
                  />
                </div>
              ))}
              <div className="flex gap-2">
                <Button
                  size="sm"
                  className="h-6 text-[10px]"
                  onClick={() => saveConfig(p.name)}
                >
                  保存
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-6 text-[10px]"
                  onClick={cancelEdit}
                >
                  取消
                </Button>
              </div>
            </div>
          )}
        </div>
      ))}
      {plugins.length === 0 && (
        <p className="text-muted-foreground text-center py-4">未找到插件。</p>
      )}
    </div>
  );
}
