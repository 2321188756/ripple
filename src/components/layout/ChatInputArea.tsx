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

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "0px";
    textarea.style.height = `${Math.min(textarea.scrollHeight, 160)}px`;
  }, [input]);

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
          "border-t border-border bg-glass p-3 transition-colors sm:p-4",
          dragOver && "bg-primary/10 ring-1 ring-inset ring-primary/30",
        )}
        onDrop={handleDrop}
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
      >
        <div className="mx-auto max-w-4xl space-y-2.5">
          {/* 图片预览区 */}
          {images.length > 0 && (
            <div className="flex gap-2 flex-wrap">
              {images.map((img) => (
                <div key={img.id} className="group relative h-20 w-20 overflow-hidden rounded-lg border border-border bg-muted shadow-xs">
                  <button
                    type="button"
                    className="block h-full w-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    onClick={() => openPreview(img.dataUrl)}
                    aria-label={`预览图片：${img.name}`}
                  >
                    <img src={img.dataUrl} alt={img.name} className="h-full w-full object-cover" />
                  </button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    className="absolute right-0.5 top-0.5 bg-black/55 text-white opacity-0 shadow-sm transition-opacity hover:bg-black/75 hover:text-white focus-visible:opacity-100 group-hover:opacity-100"
                    onClick={() => removeImage(img.id)}
                    aria-label={`移除图片：${img.name}`}
                  >
                    <X className="h-3 w-3" />
                  </Button>
                  <span className="pointer-events-none absolute bottom-1 right-1 flex h-5 w-5 items-center justify-center rounded bg-black/45 text-white opacity-0 transition-opacity group-hover:opacity-100">
                    <Expand className="h-3 w-3" />
                  </span>
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
                  placeholder="输入消息…"
                  rows={1}
                  className="min-h-[44px] max-h-40 resize-none overflow-y-auto py-3 pr-11 text-sm shadow-xs"
                  aria-label="消息输入框"
                  aria-describedby="composer-shortcuts"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  onClick={handleSelectFile}
                  className="absolute bottom-1.5 right-1.5 text-muted-foreground"
                  aria-label="添加图片附件"
                >
                  <ImagePlus className="h-4 w-4" />
                </Button>
              </div>
            </div>
            {streaming ? (
              <Button variant="destructive" size="default" onClick={onStop} className="h-11 self-end rounded-xl px-4 shadow-sm sm:px-5">
                <Square className="mr-1.5 h-4 w-4" />停止
              </Button>
            ) : (
              <Button size="default" onClick={handleSend} disabled={!hasContent} className="h-11 self-end rounded-xl px-4 shadow-sm sm:px-5">
                <Send className="mr-1.5 h-4 w-4" />发送
              </Button>
            )}
          </div>
          <div id="composer-shortcuts" className="flex items-center justify-between px-1 text-[11px] text-muted-foreground">
            <span>@ 可引用知识库 · 支持拖拽、粘贴或添加图片</span>
            <span><kbd className="rounded border border-border bg-muted px-1 py-0.5 font-sans">Enter</kbd> 发送 · <kbd className="rounded border border-border bg-muted px-1 py-0.5 font-sans">Shift + Enter</kbd> 换行</span>
          </div>
        </div>
      </div>

    </>
  );
}
