import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Sidebar } from "@/components/layout/Sidebar";
import { ChatHeader } from "@/components/layout/ChatHeader";
import { ChatInputArea } from "@/components/layout/ChatInputArea";
import { ErrorBanner } from "@/components/layout/ErrorBanner";
import { VirtualMessageList } from "@/components/chat/VirtualMessageList";
import { ImagePreview } from "@/components/common/ImagePreview";
import { ApprovalDialog } from "@/components/common/ApprovalDialog";
import { ThemeWorkshop } from "@/components/theme/ThemeWorkshop";
import { openSettingsWindow } from "@/lib/openSettings";
import { useChatStore } from "@/stores/chatStore";
import { useAgentStore } from "@/stores/agentStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useKBStore } from "@/stores/kbStore";
import { useIpcStatus } from "@/hooks/useIpcStatus";
import { useStreamEvents } from "@/hooks/useStreamEvents";
import { useTheme } from "@/hooks/useTheme";

function App() {
  // 精确订阅：不订阅 streamingText（每 token 变化）。流式文本由 VirtualMessageList
  // 自行从 store 订阅；App 只需 isStreaming 布尔（仅 start/end 翻转），避免每 token 全树重渲染。
  const activeId = useChatStore((s) => s.activeId);
  const streaming = useChatStore((s) => s.streamingText !== null);
  const error = useChatStore((s) => s.error);
  const hasMessages = useChatStore((s) =>
    s.activeId ? (s.messages[s.activeId]?.length ?? 0) > 0 : false,
  );
  const loadConversations = useChatStore((s) => s.loadConversations);
  const createConversation = useChatStore((s) => s.createConversation);
  const switchConversation = useChatStore((s) => s.switchConversation);
  const restoreLastActive = useChatStore((s) => s.restoreLastActive);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const stopGeneration = useChatStore((s) => s.stopGeneration);
  const clearError = useChatStore((s) => s.clearError);
  const retry = useChatStore((s) => s.retry);
  const canRetry = useChatStore((s) => s.lastRequest !== null);
  const toggleAgentMode = useChatStore((s) => s.toggleAgentMode);
  const selectedAgent = useAgentStore((s) => s.selectedAgent);
  const loadAgents = useAgentStore((s) => s.loadAgents);
  const settingsLoad = useSettingsStore((s) => s.load);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const ipcOk = useIpcStatus();
  const [previewImg, setPreviewImg] = useState<string | null>(null);
  const [workshopOpen, setWorkshopOpen] = useState(false);
  useStreamEvents();
  const { theme, setTheme, isDark } = useTheme();

  // 初始化
  useEffect(() => {
    settingsLoad();
    loadAgents();
    useKBStore.getState().loadKBs();
    loadConversations(); // 首次加载无 agent 过滤的全局对话
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 跨窗口同步：独立设置窗口改动 apiKey/知识库后，通知主窗口刷新缓存
  // （两窗口是独立 JS 上下文，store 不共享）
  useEffect(() => {
    const un1 = listen("ripple:settings-changed", () => {
      useSettingsStore.getState().load();
      useKBStore.getState().loadKBs();
    });
    // 设置窗口导入对话后，刷新会话列表
    const un2 = listen("ripple:conversations-changed", () => {
      useChatStore.getState().loadConversations();
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);

  // Agent 选中时自动开启 agent 模式（仅在「无 agent → 有 agent」首次选中时，
  // 避免用户手动关闭后切换 agent 又被强制开启）
  const prevAgentRef = useRef<string | null>(null);
  useEffect(() => {
    const prev = prevAgentRef.current;
    prevAgentRef.current = selectedAgent?.id ?? null;
    if (selectedAgent && !prev && !useChatStore.getState().agentMode) {
      toggleAgentMode();
    }
  }, [selectedAgent?.id, toggleAgentMode]);

  // 禁用浏览器默认右键菜单
  useEffect(() => {
    const handler = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", handler);
    return () => document.removeEventListener("contextmenu", handler);
  }, []);

  // 文件拖放：阻止浏览器默认行为
  useEffect(() => {
    const preventNav = (e: DragEvent) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); } };
    document.addEventListener("dragover", preventNav, false);
    document.addEventListener("drop", preventNav, false);
    return () => {
      document.removeEventListener("dragover", preventNav, false);
      document.removeEventListener("drop", preventNav, false);
    };
  }, []);

  // 全局图片预览事件（确保在 App 层渲染，不受子元素 transform 影响）
  useEffect(() => {
    const handler = (e: Event) => { setPreviewImg((e as CustomEvent).detail?.url || null); };
    window.addEventListener("ripple:preview-image", handler);
    return () => window.removeEventListener("ripple:preview-image", handler);
  }, []);

  // 全局快捷键
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.isComposing) return; // IME 组字中不触发快捷键
      const ctrl = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase(); // 兼容 CapsLock / Shift 下的 uppercase
      if (ctrl && k === "n") {
        e.preventDefault();
        createConversation(selectedAgent?.id).then((id) => {
          if (id) switchConversation(id, selectedAgent?.id);
        });
      } else if (ctrl && k === "k") {
        e.preventDefault();
        document.querySelector<HTMLInputElement>("input[placeholder*='搜索']")?.focus();
      } else if (ctrl && k === ",") {
        e.preventDefault();
        openSettingsWindow();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selectedAgent?.id, createConversation, switchConversation]);

  // Agent 切换时加载对应会话，并恢复上次活跃的对话
  useEffect(() => {
    if (!selectedAgent) return;
    const agentId = selectedAgent.id;
    let cancelled = false;
    const doRestore = async () => {
      await loadConversations(agentId);
      // 快速 A→B 切换时，A 的 loadConversations 可能晚于 B 完成。
      // 若 agent 已切走则放弃本次 restore，避免把 B 的会话 id 写入 lastActivePerAgent[A]。
      if (cancelled || useAgentStore.getState().selectedAgent?.id !== agentId) return;
      await restoreLastActive(agentId);
    };
    doRestore();
    return () => { cancelled = true; };
  }, [selectedAgent?.id, loadConversations, restoreLastActive]);

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-screen bg-background text-foreground">
        <Sidebar ipcOk={ipcOk} onOpenSettings={openSettingsWindow} />

        <main role="main" className="flex-1 flex flex-col min-w-0">
          <ChatHeader
            activeId={activeId}
            hasMessages={hasMessages}
            onExportError={(msg) => useChatStore.getState().setError(msg)}
            theme={theme}
            onThemeChange={setTheme}
            onOpenWorkshop={() => setWorkshopOpen(true)}
            isDark={isDark}
          />

          <VirtualMessageList
            messagesEndRef={messagesEndRef}
          />

          <ErrorBanner error={error} onDismiss={clearError} onRetry={canRetry ? retry : undefined} />

          <ChatInputArea
            streaming={streaming}
            onSend={(text, images) => sendMessage(text, images)}
            onStop={stopGeneration}
          />
        </main>

        {previewImg && (
          <ImagePreview src={previewImg} onClose={() => setPreviewImg(null)} />
        )}

        <ApprovalDialog />

        <ThemeWorkshop open={workshopOpen} onOpenChange={setWorkshopOpen} />
      </div>
    </TooltipProvider>
  );
}

export default App;
