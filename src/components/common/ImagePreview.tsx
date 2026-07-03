import { useCallback, useRef, useState } from "react";
import { ZoomIn, ZoomOut, X } from "lucide-react";

interface ImagePreviewProps {
  src: string;
  onClose: () => void;
}

/** 图片预览：App 层渲染，缩放 + 滚轮 + 拖动 */
export function ImagePreview({ src, onClose }: ImagePreviewProps) {
  const [scale, setScale] = useState(1);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const drag = useRef({ on: false, sx: 0, sy: 0, px: 0, py: 0 });

  const zoomTo = useCallback((s: number) => setScale(Math.max(0.1, Math.min(5, s))), []);
  const zoomIn = () => zoomTo(scale * 1.25);
  const zoomOut = () => zoomTo(scale / 1.25);
  const resetFit = () => { setScale(1); setPos({ x: 0, y: 0 }); };

  // 拖动
  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    drag.current = { on: true, sx: e.clientX, sy: e.clientY, px: pos.x, py: pos.y };
  };
  const handleMouseMove = (e: React.MouseEvent) => {
    if (!drag.current.on) return;
    setPos({ x: drag.current.px + e.clientX - drag.current.sx, y: drag.current.py + e.clientY - drag.current.sy });
  };
  const handleMouseUp = () => { drag.current.on = false; };

  // 滚轮缩放
  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    e.stopPropagation();
    zoomTo(scale - e.deltaY * 0.002);
  };

  return (
    <div
      className="fixed inset-0 z-[9999] bg-black overflow-hidden select-none"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
      onWheel={handleWheel}
    >
      {/* 工具栏 */}
      <div className="absolute top-0 left-0 right-0 z-10 flex items-center justify-between px-4 py-2 bg-zinc-900/80" style={{ height: 44 }}>
        <div className="flex items-center gap-1.5">
          <button className="w-7 h-7 flex items-center justify-center rounded text-white/70 hover:text-white hover:bg-white/10"
            onClick={(e) => { e.stopPropagation(); zoomOut(); }} title="缩小">
            <ZoomOut className="w-4 h-4" />
          </button>
          <span className="text-xs text-white/60 w-10 text-center">{Math.round(scale * 100)}%</span>
          <button className="w-7 h-7 flex items-center justify-center rounded text-white/70 hover:text-white hover:bg-white/10"
            onClick={(e) => { e.stopPropagation(); zoomIn(); }} title="放大">
            <ZoomIn className="w-4 h-4" />
          </button>
          <button className="h-7 px-2 text-xs text-white/60 hover:text-white hover:bg-white/10 rounded"
            onClick={(e) => { e.stopPropagation(); resetFit(); }}>
            适应
          </button>
        </div>
        <button className="w-7 h-7 flex items-center justify-center rounded text-white/70 hover:text-white hover:bg-white/10"
          onClick={onClose} title="关闭">
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* 图片区域 */}
      <div className="absolute inset-0 flex items-center justify-center" style={{ top: 44 }}
        onClick={() => { if (scale === 1 && pos.x === 0 && pos.y === 0) onClose(); }}>
        <img
          src={src}
          alt="preview"
          draggable={false}
          className="pointer-events-none"
          style={{
            maxWidth: scale === 1 && pos.x === 0 && pos.y === 0 ? "90%" : undefined,
            maxHeight: scale === 1 && pos.x === 0 && pos.y === 0 ? "90%" : undefined,
            objectFit: scale === 1 && pos.x === 0 && pos.y === 0 ? "contain" : undefined,
            transform: scale !== 1 || pos.x !== 0 || pos.y !== 0
              ? `translate(${pos.x}px,${pos.y}px) scale(${scale})`
              : undefined,
            transition: drag.current.on ? "none" : "transform 0.12s",
            cursor: "grab",
          }}
        />
      </div>

      {/* 提示 */}
      <div className="absolute bottom-4 left-1/2 -translate-x-1/2 text-[11px] text-white/30 pointer-events-none">
        滚轮缩放 · 拖动移动 · 点击关闭
      </div>
    </div>
  );
}
