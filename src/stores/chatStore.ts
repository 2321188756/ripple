import { create } from "zustand";
import { conversationService, messageService, chatService, logService } from "@/services";
import { useSettingsStore } from "./settingsStore";
import type {
  Conversation,
  Message,
  StreamChunkPayload,
  GenCompletePayload,
} from "@/types";

// ---------- Store ----------

interface ChatState {
  conversations: Conversation[];
  activeId: string | null;
  messages: Record<string, Message[]>;
  toolEvents: Record<string, any[]>; // conversation_id → tool call events
  agentMode: boolean;
  streamingText: string | null;
  streamingMsgId: string | null;
  loading: boolean;
  error: string | null;
  /** 记住每个 Agent 上次活跃的会话 */
  lastActivePerAgent: Record<string, string>;

  setError: (msg: string) => void;
  clearError: () => void;
  addToolEvent: (conversationId: string, event: any) => void;
  toggleAgentMode: () => void;

  // actions
  loadConversations: (agentId?: string) => Promise<void>;
  createConversation: (agentId?: string) => Promise<string>;
  switchConversation: (id: string, agentId?: string) => Promise<void>;
  /** 选中 Agent 后恢复其上次活跃会话，没有则选最新 */
  restoreLastActive: (agentId: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  appendToStreaming: (chunk: StreamChunkPayload) => void;
  finalizeStreaming: (payload: GenCompletePayload) => void;
  clearStreaming: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  activeId: null,
  messages: {},
  toolEvents: {},
  agentMode: false,
  streamingText: null,
  streamingMsgId: null,
  loading: false,
  error: null,
  lastActivePerAgent: {},

  loadConversations: async (agentId?: string) => {
    const convos = await conversationService
      .list(agentId ? { agentId } : {})
      .catch(() => []);
    set({ conversations: convos });
  },

  createConversation: async (agentId?: string) => {
    const conv = await conversationService.create(agentId ? { agentId } : {});
    set((s) => ({ conversations: [conv, ...s.conversations] }));
    return conv.id;
  },

  switchConversation: async (id: string, agentId?: string) => {
    const state = get();
    // 记住该 Agent 的最后活跃会话
    if (agentId) {
      set({ lastActivePerAgent: { ...state.lastActivePerAgent, [agentId]: id } });
    }

    const existing = state.messages[id];
    if (existing && existing.length > 0) {
      set({ activeId: id, streamingText: null, streamingMsgId: null });
      return;
    }
    set({ activeId: id, streamingText: null, streamingMsgId: null });
    const msgs = await messageService.list(id).catch(() => []);
    if (msgs.length > 0) {
      set((s) => ({ messages: { ...s.messages, [id]: msgs } }));
    }
  },

  restoreLastActive: async (agentId: string) => {
    const state = get();
    const lastId = state.lastActivePerAgent[agentId];

    if (lastId) {
      // 确认这个会话还在当前列表中
      const exists = state.conversations.some((c) => c.id === lastId);
      if (exists) {
        await state.switchConversation(lastId, agentId);
        return;
      }
    }

    // 没有记忆或已删除 → 选最新的
    const sorted = [...state.conversations].sort(
      (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
    );
    if (sorted.length > 0) {
      await state.switchConversation(sorted[0].id, agentId);
    }
  },

  sendMessage: async (content: string) => {
    const state = get();
    if (state.streamingText !== null) {
      await logService.log("warn", "sendMessage: already streaming, ignored");
      return;
    }
    await logService.log("info", `sendMessage called: activeId=${state.activeId}`);
    let aid = state.activeId;
    if (!aid) {
      await logService.log("warn", "sendMessage: no active conversation, creating one");
      try {
        const id = await state.createConversation();
        await state.switchConversation(id);
        aid = id;
      } catch (e) {
        await logService.log("error", `sendMessage: auto-create conversation failed: ${e}`);
        set({ error: "Failed to create conversation." });
        return;
      }
    }

    const userMsg: Message = {
      id: crypto.randomUUID(),
      conversation_id: aid,
      role: "user",
      content: [{ type: "text", text: content }],
      created_at: new Date().toISOString(),
      token_count: null,
      metadata: {},
    };

    set((s) => ({
      messages: {
        ...s.messages,
        [aid]: [...(s.messages[aid] || []), userMsg],
      },
    }));

    await logService.log("info", `send_message: conv=${aid} len=${content.length}`);
    try {
      const s = useSettingsStore.getState();
      const agentMode = get().agentMode;
      const msgId = await chatService.send({
        conversationId: aid,
        content,
        apiKey: s.apiKey,
        apiBaseUrl: s.apiBaseUrl,
        model: s.defaultModel,
        agentMode,
      });
      await logService.log("info", `send_message ok: msgId=${msgId}`);
      set({ streamingMsgId: msgId, streamingText: "" });
    } catch (err) {
      const msg = typeof err === "string" ? err : "send_message failed";
      await logService.log("error", `send_message error: ${msg}`);
      set({ error: msg });
    }
  },

  stopGeneration: async () => {
    const { activeId } = get();
    if (activeId) {
      await chatService.stop(activeId).catch(() => {});
      set({ streamingText: null, streamingMsgId: null });
    }
  },

  appendToStreaming: (chunk: StreamChunkPayload) => {
    const s = get();
    if (chunk.delta_text && s.streamingMsgId === chunk.message_id) {
      set({ streamingText: (s.streamingText || "") + chunk.delta_text });
    }
  },

  finalizeStreaming: (payload: GenCompletePayload) => {
    const { activeId, streamingText, streamingMsgId } = get();
    if (!activeId || !streamingMsgId || streamingText === null) return;

    const assistantMsg: Message = {
      id: streamingMsgId,
      conversation_id: activeId,
      role: "assistant",
      content: [{ type: "text", text: streamingText }],
      created_at: new Date().toISOString(),
      token_count: payload.usage.total_tokens,
      metadata: {},
    };

    set((s) => ({
      messages: {
        ...s.messages,
        [activeId]: [...(s.messages[activeId] || []), assistantMsg],
      },
      streamingText: null,
      streamingMsgId: null,
    }));
  },

  clearStreaming: () => {
    set({ streamingText: null, streamingMsgId: null });
  },

  setError: (msg: string) => set({ error: msg }),
  clearError: () => set({ error: null }),

  toggleAgentMode: () => set((s) => ({ agentMode: !s.agentMode })),

  addToolEvent: (conversationId: string, event: any) => {
    set((s) => {
      const existing = s.toolEvents[conversationId] || [];
      return { toolEvents: { ...s.toolEvents, [conversationId]: [...existing, event] } };
    });
  },
}));
