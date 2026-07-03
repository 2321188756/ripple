import { useMemo } from "react";
import { Plus, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ConversationListItem } from "./ConversationListItem";
import { useSearch } from "@/hooks/useSearch";
import { conversationService } from "@/services";
import { useChatStore } from "@/stores/chatStore";
import type { Conversation } from "@/types";

/** 转义后端搜索片段中的 HTML，防止消息正文里的 <script> 等被注入（XSS）。
 *  以纯文本渲染，放弃可能的 <mark> 高亮标记以换取安全性。 */
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

interface ConversationListViewProps {
  conversations: Conversation[];
  activeId: string | null;
  agentId?: string;
  onSelect: (id: string, agentId?: string) => void;
  onReload: () => void;
}

/** 侧边栏会话列表：新建按钮 + 搜索 + 列表 */
export function ConversationListView({
  conversations,
  activeId,
  agentId,
  onSelect,
  onReload,
}: ConversationListViewProps) {
  const { query, setQuery, results, showResults, execute, clear } = useSearch();

  const handleNew = async () => {
    const { createConversation, switchConversation } = useChatStore.getState();
    const id = await createConversation(agentId);
    await switchConversation(id, agentId);
  };

  const handleDelete = async (id: string) => {
    await conversationService.delete(id);
    onReload();
  };

  const handleRename = async (id: string, title: string) => {
    await conversationService.update(id, { title });
    onReload();
  };

  const handleTogglePin = async (id: string, pinned: boolean) => {
    await conversationService.update(id, { pinned: !pinned });
    onReload();
  };

  const sorted = useMemo(
    () => [...conversations].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0)),
    [conversations],
  );

  return (
    <div className="flex flex-col h-full">
      <div className="p-2 border-b border-border space-y-1.5">
        <Button
          onClick={handleNew}
          size="sm"
          className="w-full h-7 text-xs"
        >
          <Plus className="w-3.5 h-3.5 mr-1" />
          新建对话
        </Button>
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") execute();
            }}
            placeholder="搜索消息..."
            className="h-7 text-xs pl-7"
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {showResults ? (
          results.map((r, i) => (
            <div
              key={`${r.conversation_id}-${i}`}
              onClick={() => {
                onSelect(r.conversation_id, agentId);
                clear();
              }}
              className="px-3 py-2 border-b border-border text-xs cursor-pointer hover:bg-accent/60"
            >
              <div className="text-muted-foreground truncate">
                {r.role} · {new Date(r.created_at).toLocaleTimeString()}
              </div>
              <div className="text-foreground/80 line-clamp-2 whitespace-pre-wrap">
                {escapeHtml(r.snippet)}
              </div>
            </div>
          ))
        ) : (
          sorted.map((c) => (
            <ConversationListItem
              key={c.id}
              conv={c}
              active={c.id === activeId}
              onSelect={() => onSelect(c.id, agentId)}
              onDelete={() => handleDelete(c.id)}
              onRename={(title) => handleRename(c.id, title)}
              onTogglePin={() => handleTogglePin(c.id, c.pinned)}
            />
          ))
        )}
      </div>
    </div>
  );
}
