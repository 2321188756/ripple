import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import type React from "react";

interface ContextMenuItem {
  label: string;
  icon?: React.ReactNode;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  onSelect: () => void;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  position: { x: number; y: number } | null;
  onClose: () => void;
}

export function ContextMenu({ items, position, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!position) return;
    const menu = ref.current;
    const buttons = () => Array.from(menu?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)') ?? []);
    requestAnimationFrame(() => buttons()[0]?.focus());
    const pointerHandler = (event: MouseEvent) => {
      if (menu && !menu.contains(event.target as Node)) onClose();
    };
    const keyHandler = (event: KeyboardEvent) => {
      const enabled = buttons();
      if (event.key === "Escape" || event.key === "Tab") {
        onClose();
        return;
      }
      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || enabled.length === 0) return;
      event.preventDefault();
      const current = enabled.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0
        : event.key === "End" ? enabled.length - 1
        : event.key === "ArrowDown" ? (current + 1 + enabled.length) % enabled.length
        : (current - 1 + enabled.length) % enabled.length;
      enabled[next]?.focus();
    };
    const closeOnViewportChange = () => onClose();
    document.addEventListener("mousedown", pointerHandler);
    document.addEventListener("keydown", keyHandler);
    window.addEventListener("resize", closeOnViewportChange);
    return () => {
      document.removeEventListener("mousedown", pointerHandler);
      document.removeEventListener("keydown", keyHandler);
      window.removeEventListener("resize", closeOnViewportChange);
    };
  }, [position, onClose]);

  if (!position) return null;
  const adjustedX = Math.max(8, Math.min(position.x, window.innerWidth - 180));
  const adjustedY = Math.max(8, Math.min(position.y, window.innerHeight - 320));

  return (
    <div
      ref={ref}
      role="menu"
      aria-label="操作菜单"
      className="fixed z-[100] max-h-[calc(100vh-16px)] min-w-[160px] overflow-y-auto rounded-lg border border-border bg-popover py-1 shadow-xl animate-in fade-in-0 zoom-in-95"
      style={{ left: adjustedX, top: adjustedY }}
    >
      {items.map((item, index) => item.separator ? (
        <div key={index} role="separator" className="mx-2 my-1 h-px bg-border" />
      ) : (
        <button
          key={index}
          type="button"
          role="menuitem"
          tabIndex={-1}
          disabled={item.disabled}
          className={cn(
            "flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-xs transition-colors focus:bg-accent focus:outline-none",
            item.danger ? "text-destructive hover:bg-destructive/10" : "text-popover-foreground hover:bg-accent",
            item.disabled && "cursor-not-allowed opacity-40",
          )}
          onClick={() => { item.onSelect(); onClose(); }}
        >
          {item.icon && <span className="h-4 w-4 shrink-0">{item.icon}</span>}
          <span className="flex-1">{item.label}</span>
          {item.shortcut && <span className="text-[10px] text-muted-foreground">{item.shortcut}</span>}
        </button>
      ))}
    </div>
  );
}
