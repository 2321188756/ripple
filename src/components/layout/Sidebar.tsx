import { Settings as SettingsIcon, PanelLeftClose, PanelLeft } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { IpcStatusIndicator } from "@/components/common/IpcStatusIndicator";
import { AgentListView } from "@/components/sidebar/AgentListView";
import { ConversationListView } from "@/components/sidebar/ConversationListView";
import { AgentEditorPanel } from "@/components/sidebar/AgentEditorPanel";
import { useAgentStore } from "@/stores/agentStore";
import { useChatStore } from "@/stores/chatStore";
import { useUIStore } from "@/stores/uiStore";
import type { SidebarTab } from "@/types/theme";

interface SidebarProps {
  ipcOk: boolean | null;
  onOpenSettings: () => void;
}

/** 侧边栏：Agent 列表 + 当前 Agent 的会话列表 + Agent 编辑 */
export function Sidebar({ ipcOk, onOpenSettings }: SidebarProps) {
  const selectedAgent = useAgentStore((s) => s.selectedAgent);
  const sidebarTab = useAgentStore((s) => s.sidebarTab);
  const setSidebarTab = useAgentStore((s) => s.setSidebarTab);
  // 精确订阅：避免订阅 streamingText（每 token 变化）导致侧边栏连同整个会话列表重渲染
  const conversations = useChatStore((s) => s.conversations);
  const activeId = useChatStore((s) => s.activeId);
  const loadConversations = useChatStore((s) => s.loadConversations);
  const switchConversation = useChatStore((s) => s.switchConversation);
  const sidebarOpen = useUIStore((s) => s.sidebarOpen);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);

  const tabs: { key: SidebarTab; label: string }[] = [
    { key: "agents", label: "Agent" },
    ...(selectedAgent
      ? [
          { key: "chats" as SidebarTab, label: "会话" },
          { key: "settings" as SidebarTab, label: "编辑" },
        ]
      : []),
  ];

  return (
    <aside
      role="complementary"
      aria-label="侧边栏"
      className={cn(
        "border-r border-sidebar-border flex flex-col bg-sidebar text-sidebar-foreground transition-all duration-200",
        sidebarOpen ? "w-64" : "w-12",
      )}
    >
      {sidebarOpen ? (
        <>
          {/* 顶部：Logo + 折叠按钮 */}
          <div className="flex items-center justify-between px-3 py-2.5 border-b border-sidebar-border">
            <div className="flex items-center gap-2">
              <div className="w-5 h-5 rounded-md bg-sidebar-primary flex items-center justify-center">
                <span className="text-sidebar-primary-foreground text-[9px] font-bold">R</span>
              </div>
              <span className="text-xs font-semibold tracking-tight">Ripple</span>
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6"
                  onClick={toggleSidebar}
                  aria-label="收起侧边栏"
                >
                  <PanelLeftClose className="w-3.5 h-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>收起侧边栏</TooltipContent>
            </Tooltip>
          </div>

          {/* Tab bar */}
          {tabs.length > 1 && (
            <div className="flex border-b border-sidebar-border px-2 pt-1">
              {tabs.map((t) => (
                <button
                  key={t.key}
                  onClick={() => setSidebarTab(t.key)}
                  className={cn(
                    "flex-1 py-2 text-[11px] font-medium text-center transition-colors rounded-t-md",
                    sidebarTab === t.key
                      ? "text-sidebar-primary bg-sidebar-accent"
                      : "text-muted-foreground hover:text-foreground hover:bg-sidebar-accent/50",
                  )}
                >
                  {t.label}
                </button>
              ))}
            </div>
          )}

          {/* Content */}
          <div className="flex-1 min-h-0 flex flex-col">
            {sidebarTab === "agents" && (
              <AgentListView selectedAgent={selectedAgent} onSelect={(a) => useAgentStore.getState().selectAgent(a)} />
            )}
            {sidebarTab === "chats" && selectedAgent && (
              <ConversationListView
                conversations={conversations}
                activeId={activeId}
                agentId={selectedAgent.id}
                onSelect={(id, aid) => switchConversation(id, aid)}
                onReload={() => loadConversations(selectedAgent.id)}
              />
            )}
            {sidebarTab === "settings" && selectedAgent && (
              <ScrollArea className="flex-1">
                <AgentEditorPanel key={selectedAgent.id} agent={selectedAgent} />
              </ScrollArea>
            )}
          </div>

          <Separator />

          {/* Bottom bar */}
          <div className="px-3 py-2 flex items-center justify-between">
            <Button
              variant="ghost"
              size="sm"
              onClick={onOpenSettings}
              className="h-7 text-[11px] text-muted-foreground hover:text-foreground px-2"
            >
              <SettingsIcon className="w-3 h-3 mr-1.5" />
              全局设置
            </Button>
            <IpcStatusIndicator status={ipcOk} />
          </div>
        </>
      ) : (
        /* 折叠状态：只显示图标 */
        <div className="flex flex-col items-center py-3 gap-3">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={toggleSidebar}
                aria-label="展开侧边栏"
              >
                <PanelLeft className="w-4 h-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right">展开侧边栏</TooltipContent>
          </Tooltip>
          <Separator />
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={onOpenSettings}
                aria-label="设置"
              >
                <SettingsIcon className="w-3.5 h-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right">全局设置</TooltipContent>
          </Tooltip>
          <IpcStatusIndicator status={ipcOk} />
        </div>
      )}
    </aside>
  );
}
