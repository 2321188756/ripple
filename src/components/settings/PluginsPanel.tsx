import { AlertCircle, Settings2, Wrench } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { usePlugins, isSensitiveConfigField } from "@/hooks/usePlugins";

export function PluginsPanel() {
  const pluginState = usePlugins();
  const activePlugin = pluginState.plugins.find((p) => p.name === pluginState.editing);

  return (
    <div className="space-y-3 text-xs">
      <p className="text-muted-foreground">插件从 <code className="text-foreground/80">plugins/</code> 目录加载。</p>
      {pluginState.error && (
        <div role="alert" className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 p-2 text-destructive">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>{pluginState.error}</span>
        </div>
      )}
      {pluginState.loading && <p className="py-4 text-center text-muted-foreground">正在扫描插件…</p>}
      {!pluginState.loading && pluginState.plugins.map((plugin) => (
        <div key={plugin.name} className="rounded-lg border border-border bg-card p-3 text-card-foreground">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="font-medium text-sm">{plugin.name}</span>
                <span className="text-muted-foreground">v{plugin.version}</span>
                <Badge variant={plugin.mode === "daemon" ? "secondary" : plugin.mode === "transform" ? "warning" : "success"}>{plugin.mode || "tool"}</Badge>
                <Badge variant="outline">{plugin.runtime}</Badge>
              </div>
              {plugin.description && <p className="mt-1 text-muted-foreground">{plugin.description}</p>}
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {plugin.config_schema?.properties && (
                <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" onClick={() => void pluginState.openConfig(plugin.name)}>
                  <Settings2 className="mr-1 h-3 w-3" />详情
                </Button>
              )}
              <Switch checked={plugin.enabled} aria-label={`${plugin.enabled ? "停用" : "启用"}${plugin.name}`} onCheckedChange={(enabled) => void pluginState.toggle(plugin.name, enabled)} />
            </div>
          </div>
          {!!plugin.tools?.length && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {plugin.tools.map((tool) => <Badge key={tool.name} variant="secondary" className="font-normal"><Wrench className="mr-1 h-2.5 w-2.5" />{tool.name}</Badge>)}
            </div>
          )}
        </div>
      ))}
      {!pluginState.loading && pluginState.plugins.length === 0 && <p className="py-4 text-center text-muted-foreground">未找到插件。</p>}

      <Dialog open={Boolean(activePlugin)} onOpenChange={(open) => { if (!open) pluginState.cancelEdit(); }}>
        <DialogContent className="sm:max-w-md">
          {activePlugin && (
            <>
              <DialogHeader>
                <DialogTitle>{activePlugin.name} 配置</DialogTitle>
                <DialogDescription>敏感字段以掩码显示；不修改掩码将保留原值。</DialogDescription>
              </DialogHeader>
              <div className="space-y-4">
                {Object.entries(activePlugin.config_schema?.properties ?? {}).map(([key, property]) => {
                  const value = pluginState.editVals[key];
                  const label = property.description || key;
                  if (property.type === "boolean") return (
                    <div key={key} className="flex items-center justify-between gap-3">
                      <Label htmlFor={`plugin-${key}`}>{label}</Label>
                      <Switch id={`plugin-${key}`} checked={Boolean(value)} onCheckedChange={(checked) => pluginState.setEditVals((current) => ({ ...current, [key]: checked }))} />
                    </div>
                  );
                  return (
                    <div key={key} className="space-y-1.5">
                      <Label htmlFor={`plugin-${key}`}>{label}</Label>
                      <Input id={`plugin-${key}`} type={isSensitiveConfigField(key, property) ? "password" : property.type === "number" || property.type === "integer" ? "number" : "text"} value={String(value ?? "")} onChange={(event) => pluginState.setEditVals((current) => ({ ...current, [key]: event.target.value }))} />
                    </div>
                  );
                })}
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={pluginState.cancelEdit}>取消</Button>
                <Button disabled={pluginState.saving} onClick={() => void pluginState.saveConfig(activePlugin.name)}>{pluginState.saving ? "保存中…" : "保存"}</Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
