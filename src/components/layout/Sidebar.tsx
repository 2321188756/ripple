import { Settings as SettingsIcon, PanelLeftClose, PanelLeft } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { IpcStatusIndicator } from "@/components/common/IpcStatusIndicator";
import { AppLogo } from "@/components/common/AppLogo";
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
  variant?: "desktop" | "mobile";
  onNavigate?: () => void;
}

interface SidebarContentProps extends Omit<SidebarProps, "variant"> {
  compact: boolean;
}

/** 侧边栏内容：桌面栏与移动抽屉复用同一导航和业务连接。 */
function SidebarContent({ ipcOk, onOpenSettings, compact, onNavigate }: SidebarContentProps) {
  const selectedAgent = useAgentStore((s) => s.selectedAgent);
  const sidebarTab = useAgentStore((s) => s.sidebarTab);
  const setSidebarTab = useAgentStore((s) => s.setSidebarTab);
  // 精确订阅：避免订阅 streamingText（每 token 变化）导致侧边栏连同整个会话列表重渲染
  const conversations = useChatStore((s) => s.conversations);
  const activeId = useChatStore((s) => s.activeId);
  const loadConversations = useChatStore((s) => s.loadConversations);
  const switchConversation = useChatStore((s) => s.switchConversation);
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

  if (compact) {
    return (
      <div className="flex h-full flex-col items-center gap-3 py-3">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" onClick={toggleSidebar} aria-label="展开侧边栏">
              <PanelLeft className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">展开侧边栏</TooltipContent>
        </Tooltip>
        <Separator />
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" onClick={onOpenSettings} aria-label="设置">
              <SettingsIcon className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">全局设置</TooltipContent>
        </Tooltip>
        <IpcStatusIndicator status={ipcOk} />
      </div>
    );
  }

  const handleOpenSettings = () => {
    onOpenSettings();
    onNavigate?.();
  };

  return (
    <>
      <div className="flex items-center justify-between border-b border-sidebar-border px-4 py-3">
        <div className="flex items-center gap-2">
          <AppLogo size="sm" />
          <span className="text-sm font-semibold tracking-tight">Ripple</span>
        </div>
        {!onNavigate && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-xs" onClick={toggleSidebar} aria-label="收起侧边栏">
                <PanelLeftClose className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>收起侧边栏</TooltipContent>
          </Tooltip>
        )}
      </div>

      {tabs.length > 1 && (
        <div aria-label="侧边栏视图" className="flex gap-1 border-b border-sidebar-border px-3 pt-2">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              type="button"
              aria-pressed={sidebarTab === tab.key}
              onClick={() => setSidebarTab(tab.key)}
              className={cn(
                "flex-1 rounded-t-md px-2 py-2 text-center text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring",
                sidebarTab === tab.key
                  ? "bg-sidebar-accent text-sidebar-primary"
                  : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col">
        {sidebarTab === "agents" && (
          <AgentListView
            selectedAgent={selectedAgent}
            onSelect={(agent) => {
              useAgentStore.getState().selectAgent(agent);
              onNavigate?.();
            }}
          />
        )}
        {sidebarTab === "chats" && selectedAgent && (
          <ConversationListView
            conversations={conversations}
            activeId={activeId}
            agentId={selectedAgent.id}
            onSelect={(id, agentId) => {
              switchConversation(id, agentId);
              onNavigate?.();
            }}
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
      <div className="flex items-center justify-between px-4 py-3">
        <Button variant="ghost" size="sm" onClick={handleOpenSettings} className="text-muted-foreground hover:text-foreground">
          <SettingsIcon className="mr-1.5 h-3.5 w-3.5" />
          全局设置
        </Button>
        <IpcStatusIndicator status={ipcOk} />
      </div>
    </>
  );
}

/** 侧边栏：桌面端栏位或窄窗口抽屉中的共享内容。 */
export function Sidebar({ ipcOk, onOpenSettings, variant = "desktop", onNavigate }: SidebarProps) {
  const sidebarOpen = useUIStore((s) => s.sidebarOpen);

  if (variant === "mobile") {
    return <SidebarContent ipcOk={ipcOk} onOpenSettings={onOpenSettings} compact={false} onNavigate={onNavigate} />;
  }

  return (
    <aside
      role="complementary"
      aria-label="侧边栏"
      className={cn(
        "hidden h-full shrink-0 flex-col overflow-hidden border-r border-sidebar-border bg-glass-sidebar text-sidebar-foreground md:flex",
        "transition-[width] duration-200 ease-out",
        sidebarOpen ? "w-[var(--sidebar-expanded-width)]" : "w-[var(--sidebar-collapsed-width)]",
      )}
    >
      <SidebarContent ipcOk={ipcOk} onOpenSettings={onOpenSettings} compact={!sidebarOpen} />
    </aside>
  );
}
