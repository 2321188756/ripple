import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { MODELS } from "@/lib/constants";

interface ModelSelectorProps {
  value: string;
  onChange: (model: string) => void;
  disabled?: boolean;
}

/** 模型下拉选择器 */
export function ModelSelector({ value, onChange, disabled }: ModelSelectorProps) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger className="h-7 w-44 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {MODELS.map((m) => (
          <SelectItem key={m.value} value={m.value} className="text-xs">
            {m.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
