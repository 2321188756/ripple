import { useRef, useState } from "react";
import { Send, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { MentionPopover } from "@/components/common/MentionPopover";
import { useMentionCompletion } from "@/hooks/useMentionCompletion";
import { useKBStore } from "@/stores/kbStore";

interface ChatInputAreaProps {
  streaming: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
}

/** 底部输入区：textarea + @补全 + 发送/停止 */
export function ChatInputArea({ streaming, onSend, onStop }: ChatInputAreaProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const kbList = useKBStore((s) => s.kbs);
  const mentionItems = kbList.map((k) => ({ id: k.id, label: k.name }));

  const {
    showMention,
    filtered,
    mentionIdx,
    detectMention,
    selectMention,
    handleKeyDown,
    hide,
  } = useMentionCompletion(mentionItems);

  const handleSend = () => {
    if (!input.trim()) return;
    onSend(input);
    setInput("");
    hide();
  };

  const onPick = (label: string) => {
    const next = selectMention(label, input, textareaRef.current);
    setInput(next);
  };

  return (
    <div className="p-4 border-t border-border bg-background">
      <div className="flex gap-2.5 max-w-4xl mx-auto">
        <div className="flex-1 flex flex-col relative">
          {showMention && (
            <MentionPopover
              items={filtered}
              activeIdx={mentionIdx}
              onPick={onPick}
            />
          )}
          <Textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              detectMention(e.target.value, e.target.selectionStart);
            }}
            onKeyDown={(e) => {
              if (handleKeyDown(e)) {
                if (e.key === "Enter" && filtered[mentionIdx]) onPick(filtered[mentionIdx].label);
                return;
              }
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="输入消息，Enter 发送，Shift+Enter 换行，@ 引用知识库..."
            rows={1}
            className="resize-none min-h-[44px] max-h-40 text-sm"
            aria-label="消息输入框"
          />
        </div>

        {streaming ? (
          <Button
            variant="destructive"
            size="default"
            onClick={onStop}
            className="self-end h-11 px-5 rounded-xl shadow-sm"
          >
            <Square className="w-4 h-4 mr-1.5" />
            停止
          </Button>
        ) : (
          <Button
            size="default"
            onClick={handleSend}
            disabled={!input.trim()}
            className="self-end h-11 px-5 rounded-xl shadow-sm"
          >
            <Send className="w-4 h-4 mr-1.5" />
            发送
          </Button>
        )}
      </div>
    </div>
  );
}
