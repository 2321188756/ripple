import { useEffect, useState } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { MODELS } from "@/lib/constants";
import { settingsService } from "@/services/settings.service";

interface ModelSelectorProps {
  value: string;
  onChange: (model: string) => void;
  disabled?: boolean;
}

/** 模型下拉选择器：动态从 newapi /v1/models 拉取，失败回退到内置列表。 */
export function ModelSelector({ value, onChange, disabled }: ModelSelectorProps) {
  const [models, setModels] = useState<{ value: string; label: string }[]>(
    () => MODELS.map((m) => ({ value: m.value, label: m.label }))
  );

  useEffect(() => {
    settingsService
      .listAvailableModels()
      .then((ids) => {
        if (ids.length > 0) {
          setModels(ids.map((id) => ({ value: id, label: id })));
        }
      })
      .catch(() => {
        // 拉取失败保持内置列表
      });
  }, []);

  // 当前值不在列表中时，追加一项避免 Select 显示空
  const options = models.some((m) => m.value === value)
    ? models
    : [{ value, label: value }, ...models];

  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger className="h-7 w-48 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((m) => (
          <SelectItem key={m.value} value={m.value} className="text-xs">
            {m.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
