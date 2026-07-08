import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Send, Square, X, ImagePlus, Expand } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { MentionPopover } from "@/components/common/MentionPopover";
import { useMentionCompletion } from "@/hooks/useMentionCompletion";
import { useKBStore } from "@/stores/kbStore";
import { cn } from "@/lib/utils";

interface ImageItem {
  id: string;
  dataUrl: string;
  name: string;
}

interface ChatInputAreaProps {
  streaming: boolean;
  onSend: (text: string, images?: string[]) => void;
  onStop: () => void;
}

/** 底部输入区：图片缩略图 + textarea + @补全 + 发送/停止 */
export function ChatInputArea({ streaming, onSend, onStop }: ChatInputAreaProps) {
  const [input, setInput] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const openPreview = useCallback((url: string) => {
    window.dispatchEvent(new CustomEvent("ripple:preview-image", { detail: { url } }));
  }, []);
  const [dragOver, setDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const kbList = useKBStore((s) => s.kbs);
  const mentionItems = useMemo(() => kbList.map((k) => ({ id: k.id, label: k.name })), [kbList]);

  const {
    showMention, filtered, mentionIdx, detectMention, selectMention, handleKeyDown, hide,
  } = useMentionCompletion(mentionItems);

  const readFile = useCallback((file: File) => {
    if (!file.type.startsWith("image/")) return;
    const reader = new FileReader();
    reader.onload = () => {
      setImages((prev) => [...prev, { id: crypto.randomUUID(), dataUrl: reader.result as string, name: file.name || "image" }]);
    };
    reader.readAsDataURL(file);
  }, []);

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (let i = 0; i < items.length; i++) {
      if (items[i].type.startsWith("image/")) {
        const file = items[i].getAsFile();
        if (file) readFile(file);
      }
    }
  }, [readFile]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const files = e.dataTransfer?.files;
    if (!files) return;
    for (let i = 0; i < files.length; i++) {
      readFile(files[i]);
    }
  }, [readFile]);

  // 监听全局自定义文件拖拽事件
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.files) {
        detail.files.forEach((f: File) => readFile(f));
      }
    };
    window.addEventListener("ripple:files-dropped", handler);
    return () => window.removeEventListener("ripple:files-dropped", handler);
  }, [readFile]);

  const handleSelectFile = () => fileInputRef.current?.click();
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    for (let i = 0; i < files.length; i++) readFile(files[i]);
    e.target.value = "";
  };

  const removeImage = (id: string) => setImages((prev) => prev.filter((img) => img.id !== id));

  const handleSend = () => {
    // 流式生成中不发送（发送按钮此时已变为停止按钮，但 textarea 的 Enter 仍会触发本函数，
    // 若不拦截会调用 onSend→sendMessage(早返回) 后无条件清空输入，导致用户输入凭空消失）。
    if (streaming) return;
    if (!input.trim() && images.length === 0) return;
    onSend(input, images.length > 0 ? images.map((img) => img.dataUrl) : undefined);
    setInput("");
    setImages([]);
    hide();
  };

  const onPick = (label: string) => {
    const next = selectMention(label, input, textareaRef.current);
    setInput(next);
  };

  const hasContent = input.trim().length > 0 || images.length > 0;

  return (
    <>
      <div
        className={cn(
          "p-4 border-t border-border bg-background transition-colors",
          dragOver && "bg-primary/5",
        )}
        onDrop={handleDrop}
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
      >
        <div className="max-w-4xl mx-auto space-y-2">
          {/* 图片预览区 */}
          {images.length > 0 && (
            <div className="flex gap-2 flex-wrap">
              {images.map((img) => (
                <div key={img.id} className="group relative w-20 h-20 rounded-lg overflow-hidden border border-border bg-muted">
                  <img
                    src={img.dataUrl}
                    alt={img.name}
                    className="w-full h-full object-cover cursor-pointer"
                    onClick={() => openPreview(img.dataUrl)}
                  />
                  <button
                    className="absolute top-0.5 right-0.5 w-5 h-5 rounded-full bg-black/50 text-white flex items-center justify-center
                               opacity-0 group-hover:opacity-100 transition-opacity hover:bg-black/70"
                    onClick={() => removeImage(img.id)}
                  >
                    <X className="w-3 h-3" />
                  </button>
                  <div className="absolute bottom-0.5 right-0.5 w-4 h-4 rounded bg-black/40 text-white flex items-center justify-center
                                  opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                       onClick={() => openPreview(img.dataUrl)}>
                    <Expand className="w-2.5 h-2.5" />
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* 输入区 */}
          <div className="flex gap-2.5">
            <div className="flex-1 flex flex-col relative">
              {showMention && <MentionPopover items={filtered} activeIdx={mentionIdx} onPick={onPick} />}
              <input ref={fileInputRef} type="file" accept="image/*" multiple className="hidden" onChange={handleFileChange} />
              <div className="relative">
                <Textarea
                  ref={textareaRef}
                  value={input}
                  onChange={(e) => {
                    setInput(e.target.value);
                    detectMention(e.target.value, e.target.selectionStart);
                  }}
                  onPaste={handlePaste}
                  onKeyDown={(e) => {
                    if (handleKeyDown(e)) {
                      if (e.key === "Enter" && filtered[mentionIdx]) onPick(filtered[mentionIdx].label);
                      return;
                    }
                    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
                  }}
                  placeholder="Enter 发送，Shift+Enter 换行，@ 引用知识库，拖拽/粘贴图片"
                  rows={1}
                  className="resize-none min-h-[44px] max-h-40 text-sm pr-8"
                  aria-label="消息输入框"
                />
                <button type="button" onClick={handleSelectFile}
                  className="absolute right-2 bottom-2 text-muted-foreground hover:text-foreground transition-colors" title="添加图片">
                  <ImagePlus className="w-4 h-4" />
                </button>
              </div>
            </div>
            {streaming ? (
              <Button variant="destructive" size="default" onClick={onStop} className="self-end h-11 px-5 rounded-xl shadow-sm">
                <Square className="w-4 h-4 mr-1.5" />停止
              </Button>
            ) : (
              <Button size="default" onClick={handleSend} disabled={!hasContent} className="self-end h-11 px-5 rounded-xl shadow-sm">
                <Send className="w-4 h-4 mr-1.5" />发送
              </Button>
            )}
          </div>
        </div>
      </div>

    </>
  );
}
