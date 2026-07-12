import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Check, X, Trash, RefreshCw, Pencil, ArrowDown } from "lucide-react";
import { MessageBubble } from "./MessageBubble";
import { StreamingMessage } from "./StreamingMessage";
import { EmptyChatPlaceholder } from "./EmptyChatPlaceholder";
import { ContextMenu } from "@/components/common/ContextMenu";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useChatStore } from "@/stores/chatStore";
import type { Message } from "@/types";

interface VirtualMessageListProps {
  messagesEndRef: React.RefObject<HTMLDivElement | null>;
}

type Item =
  | { key: string; type: "msg"; data: Message }
  | { key: string; type: "stream"; data: string };

/** 虚拟滚动消息列表 + 右键菜单 + 内联编辑 */
export const VirtualMessageList = memo(function VirtualMessageList({
  messagesEndRef,
}: VirtualMessageListProps) {
  // 精确订阅当前对话的消息/工具事件/流式文本。流式文本每 token 变化只重渲染本组件，
  // 不再经 App 向下传导导致 Sidebar/ChatHeader/ChatInputArea 全树重渲染。
  const messages = useChatStore((s) => (s.activeId ? s.messages[s.activeId] : undefined)) ?? [];
  const streamingText = useChatStore((s) => s.streamingText);

  // 右键菜单
  const [ctxPos, setCtxPos] = useState<{ x: number; y: number } | null>(null);
  const [ctxMsg, setCtxMsg] = useState<{ id: string; role: string; convId: string } | null>(null);

  // 内联编辑
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState("");

  const closeCtx = useCallback(() => { setCtxPos(null); setCtxMsg(null); }, []);

  const handleContextMenu = useCallback((e: React.MouseEvent, msg: Message) => {
    e.preventDefault();
    // 只对 user 和 assistant 消息支持右键
    if (msg.role === "system" || msg.role === "tool") return;
    setCtxPos({ x: e.clientX, y: e.clientY });
    setCtxMsg({ id: msg.id, role: msg.role, convId: msg.conversation_id });
  }, []);

  const handleRegenerate = useCallback(() => {
    if (!ctxMsg) return;
    useChatStore.getState().regenerate(ctxMsg.id, ctxMsg.convId);
    closeCtx();
  }, [ctxMsg, closeCtx]);

  const handleDelete = useCallback(() => {
    if (!ctxMsg) return;
    if (window.confirm("确定删除此消息及其后的所有消息吗？")) {
      useChatStore.getState().deleteMessage(ctxMsg.id, ctxMsg.convId);
    }
    closeCtx();
  }, [ctxMsg, closeCtx]);

  const handleEdit = useCallback(() => {
    if (!ctxMsg) return;
    const msgs = useChatStore.getState().messages[ctxMsg.convId] || [];
    const msg = msgs.find((m) => m.id === ctxMsg.id);
    const text = msg?.content?.[0]?.type === "text" ? msg.content[0].text : "";
    setEditingId(ctxMsg.id);
    setEditText(text);
    closeCtx();
  }, [ctxMsg, closeCtx]);

  const handleSaveEdit = useCallback(async () => {
    if (!editingId) return;
    await useChatStore.getState().updateMessage(editingId, editText);
    setEditingId(null);
    setEditText("");
    // 编辑后自动触发 regenerate（只针对 user 消息）
    const msg = messages.find((m) => m.id === editingId);
    if (msg?.role === "user") {
      const convId = msg.conversation_id;
      useChatStore.getState().regenerate(editingId, convId);
    }
  }, [editingId, editText, messages]);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditText("");
  }, []);

  const items = useMemo<Item[]>(() => {
    const list: Item[] = [];
    for (let i = 0; i < messages.length; i++) {
      list.push({ key: messages[i].id, type: "msg", data: messages[i] });
    }
    // 工具结果已嵌入到 AI 消息的 HTML 中（后端追加），不再渲染独立 ToolCallCard
    if (streamingText !== null) {
      list.push({ key: "__stream__", type: "stream", data: streamingText });
    }
    return list;
  }, [messages, streamingText]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 80,
    overscan: 10,
  });

  useEffect(() => {
    if (autoScroll && items.length > 0) {
      virtualizer.scrollToIndex(items.length - 1, { align: "end" });
    }
  }, [items.length, streamingText, autoScroll, virtualizer]);

  const jumpToLatest = useCallback(() => {
    if (items.length === 0) return;
    setAutoScroll(true);
    virtualizer.scrollToIndex(items.length - 1, { align: "end" });
  }, [items.length, virtualizer]);

  if (messages.length === 0 && streamingText === null) {
    return <EmptyChatPlaceholder />;
  }

  return (
    <div className="relative flex min-h-0 flex-1">
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto"
        onScroll={() => {
          const el = scrollRef.current;
          if (!el) return;
          const next = el.scrollHeight - el.scrollTop - el.clientHeight < 150;
          setAutoScroll((prev) => (prev === next ? prev : next));
        }}
      >
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
                onContextMenu={item.type === "msg" ? (e) => handleContextMenu(e, item.data) : undefined}
              >
                {item.type === "msg" ? (
                  editingId === item.data.id ? (
                    <div className="px-4 py-1.5">
                      <div className="flex gap-2">
                        <Textarea
                          autoFocus
                          value={editText}
                          onChange={(e) => setEditText(e.target.value)}
                          className="min-h-[60px] text-sm font-mono"
                          onKeyDown={(e) => {
                            if (e.key === "Enter" && e.shiftKey) return;
                            if (e.key === "Enter") { e.preventDefault(); handleSaveEdit(); }
                            if (e.key === "Escape") cancelEdit();
                          }}
                        />
                        <div className="flex flex-col gap-1 self-end">
                          <Button size="icon-xs" onClick={handleSaveEdit} aria-label="保存编辑">
                            <Check className="h-3.5 w-3.5" />
                          </Button>
                          <Button size="icon-xs" variant="outline" onClick={cancelEdit} aria-label="取消编辑">
                            <X className="h-3.5 w-3.5" />
                          </Button>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <MessageBubble role={item.data.role} content={item.data.content} />
                  )
                ) : (
                  <StreamingMessage text={item.data} />
                )}
              </div>
            );
          })}
        </div>
        <div ref={messagesEndRef} />
      </div>

      {!autoScroll && items.length > 0 && (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          className="absolute bottom-4 left-1/2 z-10 -translate-x-1/2 rounded-full border border-border bg-background/95 px-3 shadow-md backdrop-blur"
          onClick={jumpToLatest}
        >
          <ArrowDown className="mr-1.5 h-3.5 w-3.5" />
          回到最新消息
        </Button>
      )}

      {/* 右键菜单 */}
      <ContextMenu
        position={ctxPos}
        onClose={closeCtx}
        items={[
          {
            label: "编辑",
            icon: <Pencil className="w-3.5 h-3.5" />,
            onSelect: handleEdit,
          },
          {
            label: "重新生成",
            icon: <RefreshCw className="w-3.5 h-3.5" />,
            onSelect: handleRegenerate,
          },
          { label: "", separator: true, onSelect: () => {} },
          {
            label: "删除",
            icon: <Trash className="w-3.5 h-3.5" />,
            danger: true,
            onSelect: handleDelete,
          },
        ]}
      />
    </div>
  );
});
