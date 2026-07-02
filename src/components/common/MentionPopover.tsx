import { Library } from "lucide-react";
import { cn } from "@/lib/utils";

interface MentionItem {
  id: string;
  label: string;
}

interface MentionPopoverProps {
  items: MentionItem[];
  activeIdx: number;
  onPick: (label: string) => void;
}

/** @ 补全弹层，浮在输入框上方 */
export function MentionPopover({ items, activeIdx, onPick }: MentionPopoverProps) {
  if (items.length === 0) return null;
  return (
    <div
      role="listbox"
      aria-label="知识库补全"
      className="absolute bottom-full left-0 right-0 mb-1 bg-popover border border-border rounded-lg shadow-lg z-50 max-h-32 overflow-y-auto"
    >
      {items.map((item, idx) => (
        <div
          key={item.id}
          role="option"
          aria-selected={idx === activeIdx}
          onMouseDown={(e) => {
            e.preventDefault();
            onPick(item.label);
          }}
          className={cn(
            "px-3 py-1.5 text-xs cursor-pointer flex items-center gap-1.5",
            idx === activeIdx
              ? "bg-accent text-accent-foreground"
              : "hover:bg-accent/60",
          )}
        >
          <Library className="w-3 h-3 text-primary" />
          {item.label}
        </div>
      ))}
    </div>
  );
}
