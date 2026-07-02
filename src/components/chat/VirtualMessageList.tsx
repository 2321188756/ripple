import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { MessageBubble } from "./MessageBubble";
import { StreamingMessage } from "./StreamingMessage";
import { EmptyChatPlaceholder } from "./EmptyChatPlaceholder";
import ToolCallCard from "@/components/ToolCallCard";
import type { Message } from "@/types";

interface VirtualMessageListProps {
  messages: Message[];
  toolEvents: any[];
  streamingText: string | null;
  messagesEndRef: React.RefObject<HTMLDivElement | null>;
}

type Item =
  | { key: string; type: "msg"; data: Message }
  | { key: string; type: "tool"; data: any }
  | { key: string; type: "stream"; data: string };

/** 虚拟滚动消息列表（@tanstack/react-virtual） */
export const VirtualMessageList = memo(function VirtualMessageList({
  messages,
  toolEvents,
  streamingText,
  messagesEndRef,
}: VirtualMessageListProps) {
  const items = useMemo<Item[]>(() => {
    const list: Item[] = [];
    let toolsInserted = false;
    for (let i = 0; i < messages.length; i++) {
      list.push({ key: messages[i].id, type: "msg", data: messages[i] });
      // 最后一条用户消息后插入工具卡片
      if (
        !toolsInserted &&
        messages[i].role === "user" &&
        (i + 1 >= messages.length || messages[i + 1].role !== "user")
      ) {
        for (let ti = 0; ti < toolEvents.length; ti++) {
          list.push({ key: `tc-${ti}`, type: "tool", data: toolEvents[ti] });
        }
        toolsInserted = true;
      }
    }
    if (streamingText !== null) {
      list.push({ key: "__stream__", type: "stream", data: streamingText });
    }
    return list;
  }, [messages, toolEvents, streamingText]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (items[i]?.type === "tool" ? 100 : 80),
    overscan: 5,
  });

  useEffect(() => {
    if (autoScroll && items.length > 0) {
      virtualizer.scrollToIndex(items.length - 1, { align: "end" });
    }
  }, [items.length, streamingText, autoScroll, virtualizer]);

  if (messages.length === 0 && streamingText === null) {
    return <EmptyChatPlaceholder />;
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-y-auto"
      onScroll={() => {
        const el = scrollRef.current;
        if (!el) return;
        setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 150);
      }}
    >
      <div ref={messagesEndRef} />
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((vItem) => {
          const item = items[vItem.index];
          return (
            <div
              key={vItem.key}
              ref={virtualizer.measureElement}
              data-index={vItem.index}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${vItem.start}px)`,
              }}
            >
              {item.type === "msg" ? (
                <MessageBubble
                  role={item.data.role}
                  text={item.data.content?.[0]?.type === "text" ? item.data.content[0].text : ""}
                />
              ) : item.type === "tool" ? (
                <div className="px-4 py-1">
                  <ToolCallCard event={item.data} />
                </div>
              ) : (
                <StreamingMessage text={item.data} />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
