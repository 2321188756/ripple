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

/** 可复用的自定义右键菜单，定位在鼠标坐标 */
export function ContextMenu({ items, position, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!position) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // 延迟一帧注册，避免触发自身右键的 mousedown 关闭菜单
    const rafId = requestAnimationFrame(() => document.addEventListener("mousedown", handler));
    // cleanup 必须同时 cancelAnimationFrame：若 rAF 尚未触发就卸载，
    // 仅 removeEventListener 是 no-op，rAF 回调仍会注册一个永不移除的 handler（泄漏）。
    return () => {
      cancelAnimationFrame(rafId);
      document.removeEventListener("mousedown", handler);
    };
  }, [position, onClose]);

  useEffect(() => {
    if (!position) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [position, onClose]);

  if (!position) return null;

  // 确保菜单不超出视口
  const adjustedX = Math.min(position.x, window.innerWidth - 180);
  const adjustedY = Math.min(position.y, window.innerHeight - items.length * 36 - 8);

  return (
    <div
      ref={ref}
      role="menu"
      className="fixed z-[100] min-w-[160px] bg-popover border border-border rounded-lg shadow-xl py-1 animate-in fade-in-0 zoom-in-95"
      style={{ left: adjustedX, top: adjustedY }}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="h-px bg-border mx-2 my-1" />
        ) : (
          <button
            key={i}
            role="menuitem"
            disabled={item.disabled}
            className={cn(
              "w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-left transition-colors",
              item.danger
                ? "text-destructive hover:bg-destructive/10"
                : "text-popover-foreground hover:bg-accent",
              item.disabled && "opacity-40 cursor-not-allowed",
            )}
            onClick={() => {
              item.onSelect();
              onClose();
            }}
          >
            {item.icon && <span className="w-4 h-4 shrink-0">{item.icon}</span>}
            <span className="flex-1">{item.label}</span>
            {item.shortcut && (
              <span className="text-[10px] text-muted-foreground">{item.shortcut}</span>
            )}
          </button>
        ),
      )}
    </div>
  );
}
