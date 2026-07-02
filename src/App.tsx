import { useEffect, useRef } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Sidebar } from "@/components/layout/Sidebar";
import { ChatHeader } from "@/components/layout/ChatHeader";
import { ChatInputArea } from "@/components/layout/ChatInputArea";
import { ErrorBanner } from "@/components/layout/ErrorBanner";
import { VirtualMessageList } from "@/components/chat/VirtualMessageList";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { useChatStore } from "@/stores/chatStore";
import { useAgentStore } from "@/stores/agentStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKBStore } from "@/stores/kbStore";
import { useUIStore } from "@/stores/uiStore";
import { useIpcStatus } from "@/hooks/useIpcStatus";
import { useStreamEvents } from "@/hooks/useStreamEvents";
import { useTheme } from "@/hooks/useTheme";

function App() {
  const {
    activeId,
    messages,
    streamingText,
    loadConversations,
    createConversation,
    switchConversation,
    restoreLastActive,
    sendMessage,
    stopGeneration,
    error,
    clearError,
    toolEvents,
    toggleAgentMode,
  } = useChatStore();
  const { selectedAgent, loadAgents } = useAgentStore();
  const settings = useSettingsStore();
  const setSettingsOpen = useUIStore((s) => s.setSettingsOpen);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const ipcOk = useIpcStatus();
  useStreamEvents();
  const { theme, setTheme, isDark } = useTheme();

  // 初始化
  useEffect(() => {
    settings.load();
    loadAgents();
    useKBStore.getState().loadKBs();
    loadConversations(); // 首次加载无 agent 过滤的全局对话
  }, []);

  // Agent 选中时自动开启 agent 模式
  useEffect(() => {
    const store = useChatStore.getState();
    if (selectedAgent && !store.agentMode) {
      toggleAgentMode();
    }
  }, [selectedAgent?.id]);

  // 全局快捷键
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key === "n") {
        e.preventDefault();
        createConversation(selectedAgent?.id).then((id) => switchConversation(id, selectedAgent?.id));
      } else if (ctrl && e.key === "k") {
        e.preventDefault();
        document.querySelector<HTMLInputElement>("input[placeholder*='搜索']")?.focus();
      } else if (ctrl && e.key === ",") {
        e.preventDefault();
        setSettingsOpen(true);
      } else if (e.key === "Escape") {
        setSettingsOpen(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selectedAgent?.id, createConversation, switchConversation, setSettingsOpen]);

  // Agent 切换时加载对应会话，并恢复上次活跃的对话
  useEffect(() => {
    if (!selectedAgent) return;
    const doRestore = async () => {
      await loadConversations(selectedAgent.id);
      await restoreLastActive(selectedAgent.id);
    };
    doRestore();
  }, [selectedAgent?.id, loadConversations, restoreLastActive]);

  const currentMessages = activeId ? messages[activeId] || [] : [];
  const currentToolEvents = activeId ? toolEvents[activeId] || [] : [];

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-screen bg-background text-foreground">
        <Sidebar ipcOk={ipcOk} onOpenSettings={() => setSettingsOpen(true)} />

        <main role="main" className="flex-1 flex flex-col min-w-0">
          <ChatHeader
            activeId={activeId}
            hasMessages={currentMessages.length > 0}
            onExportError={(msg) => useChatStore.getState().setError(msg)}
            theme={theme}
            onThemeChange={setTheme}
            isDark={isDark}
          />

          <VirtualMessageList
            messages={currentMessages}
            toolEvents={currentToolEvents}
            streamingText={streamingText}
            messagesEndRef={messagesEndRef}
          />

          <ErrorBanner error={error} onDismiss={clearError} />

          <ChatInputArea
            streaming={streamingText !== null}
            onSend={sendMessage}
            onStop={stopGeneration}
          />
        </main>

        <SettingsDialog />
      </div>
    </TooltipProvider>
  );
}

export default App;
