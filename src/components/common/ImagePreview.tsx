import { useCallback, useRef, useState } from "react";
import { ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";

interface ImagePreviewProps {
  src: string;
  onClose: () => void;
}

/** 图片预览：App 层 Dialog，支持键盘缩放、滚轮和 pointer 拖动。 */
export function ImagePreview({ src, onClose }: ImagePreviewProps) {
  const [scale, setScale] = useState(1);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const drag = useRef({ active: false, moved: false, pointerId: 0, startX: 0, startY: 0, originX: 0, originY: 0 });

  const zoomTo = useCallback((nextScale: number) => setScale(Math.max(0.1, Math.min(5, nextScale))), []);
  const resetView = () => {
    setScale(1);
    setPosition({ x: 0, y: 0 });
  };

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    drag.current = {
      active: true,
      moved: false,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: position.x,
      originY: position.y,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current.active || event.pointerId !== drag.current.pointerId) return;
    const deltaX = event.clientX - drag.current.startX;
    const deltaY = event.clientY - drag.current.startY;
    if (Math.abs(deltaX) > 4 || Math.abs(deltaY) > 4) drag.current.moved = true;
    setPosition({
      x: drag.current.originX + deltaX,
      y: drag.current.originY + deltaY,
    });
  };

  const handlePointerEnd = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerId !== drag.current.pointerId) return;
    const shouldClose = !drag.current.moved && isAtRest;
    drag.current.active = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (shouldClose) onClose();
  };

  const handleWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    zoomTo(scale - event.deltaY * 0.002);
  };

  const isAtRest = scale === 1 && position.x === 0 && position.y === 0;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        className="z-[100] h-[100dvh] max-h-none w-screen max-w-none translate-x-[-50%] translate-y-[-50%] overflow-hidden rounded-none border-0 bg-black p-0 shadow-none"
        aria-describedby="image-preview-description"
      >
        <DialogTitle className="sr-only">图片预览</DialogTitle>
        <DialogDescription id="image-preview-description" className="sr-only">
          可使用缩放按钮、滚轮和拖动查看图片。按 Escape 关闭。
        </DialogDescription>

        <div className="absolute inset-x-0 top-0 z-10 flex h-14 items-center justify-between border-b border-white/10 bg-black/70 px-3 backdrop-blur-sm sm:px-5">
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-sm"
              className="text-white/70 hover:bg-white/10 hover:text-white"
              onClick={() => zoomTo(scale / 1.25)}
              aria-label="缩小图片"
            >
              <ZoomOut className="h-4 w-4" />
            </Button>
            <output aria-live="polite" className="w-12 text-center text-xs tabular-nums text-white/70">
              {Math.round(scale * 100)}%
            </output>
            <Button
              variant="ghost"
              size="icon-sm"
              className="text-white/70 hover:bg-white/10 hover:text-white"
              onClick={() => zoomTo(scale * 1.25)}
              aria-label="放大图片"
            >
              <ZoomIn className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-xs text-white/70 hover:bg-white/10 hover:text-white"
              onClick={resetView}
            >
              适应窗口
            </Button>
          </div>
        </div>

        <div
          className="absolute inset-x-0 bottom-0 top-14 flex touch-none items-center justify-center overflow-hidden select-none"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerEnd}
          onPointerCancel={handlePointerEnd}
          onWheel={handleWheel}
          onDoubleClick={resetView}
        >
          <img
            src={src}
            alt="预览图片"
            draggable={false}
            className="pointer-events-none max-h-[88%] max-w-[92%] object-contain"
            style={{
              transform: `translate(${position.x}px, ${position.y}px) scale(${scale})`,
              transition: drag.current.active ? "none" : "transform var(--motion-fast) var(--ease-standard)",
            }}
          />
          {isAtRest && (
            <p className="pointer-events-none absolute bottom-5 text-xs text-white/45">
              滚轮缩放 · 拖动移动 · 双击适应窗口
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
