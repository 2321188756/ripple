import { create } from "zustand";
import { conversationService, messageService, chatService, logService } from "@/services";
import { useSettingsStore } from "./settingsStore";
import { useAgentStore } from "./agentStore";
import type {
  Conversation,
  Message,
  ContentBlock,
  StreamChunkPayload,
  GenCompletePayload,
  GenErrorPayload,
  ToolCallEvent,
} from "@/types";

// ---------- Store ----------

interface ChatState {
  conversations: Conversation[];
  activeId: string | null;
  messages: Record<string, Message[]>;
  toolEvents: Record<string, ToolCallEvent[]>; // conversation_id → tool call events
  agentMode: boolean;
  streamingText: string | null;
  streamingMsgId: string | null;
  loading: boolean;
  error: string | null;
  /** 记住每个 Agent 上次活跃的会话 */
  lastActivePerAgent: Record<string, string>;

  setError: (msg: string) => void;
  clearError: () => void;
  addToolEvent: (conversationId: string, event: ToolCallEvent) => void;
  toggleAgentMode: () => void;

  // actions
  loadConversations: (agentId?: string) => Promise<void>;
  createConversation: (agentId?: string) => Promise<string>;
  switchConversation: (id: string, agentId?: string) => Promise<void>;
  /** 选中 Agent 后恢复其上次活跃会话，没有则选最新 */
  restoreLastActive: (agentId: string) => Promise<void>;
  sendMessage: (content: string, images?: string[]) => Promise<void>;
  stopGeneration: () => Promise<void>;
  /** 重生成：从指定消息重新生成 */
  regenerate: (messageId: string, conversationId?: string) => Promise<void>;
  /** 更新消息内容（编辑后自动后续需手动 regenerate） */
  updateMessage: (messageId: string, content: string) => Promise<void>;
  /** 删除消息及其后所有消息 */
  deleteMessage: (messageId: string, conversationId?: string) => Promise<void>;
  appendToStreaming: (chunk: StreamChunkPayload) => void;
  finalizeStreaming: (payload: GenCompletePayload) => void;
  /** 流式出错：保留已生成部分为助手消息，清流并设置错误 */
  handleStreamError: (payload: GenErrorPayload) => void;
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

    // 流式中切换：先停止当前对话的生成。必须在改 activeId 之前 stop，
    // 否则 stopGeneration 会用新的 activeId 停错对话、并把部分回复落到错误会话。
    if (state.streamingText !== null && state.activeId && state.activeId !== id) {
      await get().stopGeneration();
    }

    set({ activeId: id, streamingText: null, streamingMsgId: null });
    // 始终后台刷新该对话消息。早期版本命中缓存即 return，导致后端新落库的回复
    // （流式完成、或切走期间生成的）切回时永远看不到，直到重启。
    const msgs = await messageService.list(id).catch(() => []);
    set((s) => ({ messages: { ...s.messages, [id]: msgs } }));
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
    } else {
      // 该 Agent 暂无会话：清空 activeId，右侧显示空状态。
      // 否则会残留上一个 Agent 的对话，看着像"切 Agent 没生效"。
      set({ activeId: null, streamingText: null, streamingMsgId: null });
    }
  },

  sendMessage: async (content: string, images?: string[]) => {
    const state = get();
    if (state.streamingText !== null) {
      await logService.log("warn", "sendMessage: already streaming, ignored");
      return;
    }
    await logService.log("info", `sendMessage called: activeId=${state.activeId}`);
    let aid = state.activeId;
    if (!aid) {
      await logService.log("info", "sendMessage: no active conversation, creating one for current agent");
      try {
        // 用当前选中 Agent 建会话，使会话归属该 Agent（元数据带 agent_id），切回时能恢复
        const agentId = useAgentStore.getState().selectedAgent?.id;
        const id = await state.createConversation(agentId);
        await state.switchConversation(id, agentId);
        aid = id;
      } catch (e) {
        await logService.log("error", `sendMessage: auto-create conversation failed: ${e}`);
        set({ error: "Failed to create conversation." });
        return;
      }
    }

    const blocks: ContentBlock[] = [{ type: "text", text: content }];
    if (images) {
      for (const url of images) {
        blocks.push({ type: "image", url, detail: "auto" });
      }
    }
    const userMsg: Message = {
      id: crypto.randomUUID(),
      conversation_id: aid,
      role: "user",
      content: blocks,
      created_at: new Date().toISOString(),
      token_count: null,
      metadata: {},
    };

    set((s) => ({
      messages: {
        ...s.messages,
        [aid]: [...(s.messages[aid] || []), userMsg],
      },
      // 新轮次开始：清空上一轮的工具调用卡片，避免跨轮次累积后堆到错误位置
      toolEvents: { ...s.toolEvents, [aid]: [] },
    }));

    await logService.log("info", `send_message: conv=${aid} len=${content.length}`);
    // 先标记流式开始（streamingText="" 非 null），使 await 期间到达的首块能被
    // appendToStreaming 锁存 message_id，避免快模型/本地模型开头丢字。
    set({ streamingText: "", streamingMsgId: null });
    try {
      const s = useSettingsStore.getState();
      const agentMode = get().agentMode;
      const msgId = await chatService.send({
        conversationId: aid,
        content,
        images: images && images.length > 0 ? images : undefined,
        apiKey: s.apiKey,
        apiBaseUrl: s.apiBaseUrl,
        model: s.defaultModel,
        agentMode,
      });
      await logService.log("info", `send_message ok: msgId=${msgId}`);
      // 补设 msgId；若首块已锁存则同值，且不覆盖已累积的 streamingText
      set((st) => ({ streamingMsgId: msgId, streamingText: st.streamingText ?? "" }));
    } catch (err) {
      const msg = typeof err === "string" ? err : "send_message failed";
      await logService.log("error", `send_message error: ${msg}`);
      set({ error: msg, streamingText: null, streamingMsgId: null });
    }
  },

  stopGeneration: async () => {
    const { activeId, streamingText, streamingMsgId } = get();
    if (!activeId) return;
    await chatService.stop(activeId).catch(() => {});
    // 保留已生成的部分文本为助手消息（不再直接丢弃），与后端落库的部分回复同 id。
    // 后端 stop 后会发 gen-complete，但此时 streamingMsgId 已清空，finalize 会 early-return，不会重复。
    if (streamingMsgId && streamingText && streamingText.length > 0) {
      const assistantMsg: Message = {
        id: streamingMsgId,
        conversation_id: activeId,
        role: "assistant",
        content: [{ type: "text", text: streamingText }],
        created_at: new Date().toISOString(),
        token_count: null,
        metadata: {},
      };
      set((s) => ({
        messages: { ...s.messages, [activeId]: [...(s.messages[activeId] || []), assistantMsg] },
        streamingText: null,
        streamingMsgId: null,
      }));
    } else {
      set({ streamingText: null, streamingMsgId: null });
    }
  },

  appendToStreaming: (chunk: StreamChunkPayload) => {
    if (!chunk.delta_text) return;
    const s = get();
    let msgId = s.streamingMsgId;
    // 流式首块竞态：send/regenerate 的 await 期间首块可能先到，streamingMsgId 仍为 null。
    // 用首块 message_id 锁存，避免开头丢字（streamingText==="" 表示流式已开始但 msgId 未就绪）。
    if (msgId === null && s.streamingText === "") {
      msgId = chunk.message_id;
      set({ streamingMsgId: msgId });
    }
    if (msgId === chunk.message_id) {
      set({ streamingText: (s.streamingText || "") + chunk.delta_text });
    }
  },

  finalizeStreaming: (payload: GenCompletePayload) => {
    const { streamingText, streamingMsgId } = get();
    if (!streamingMsgId || streamingText === null) return;
    // 用事件携带的 conversation_id 落库，而非 activeId：流式期间用户切到别的对话时，
    // 回复仍应落到原对话，切回即可见。
    const cid = payload.conversation_id;

    const assistantMsg: Message = {
      id: streamingMsgId,
      conversation_id: cid,
      role: "assistant",
      content: [{ type: "text", text: streamingText }],
      created_at: new Date().toISOString(),
      token_count: payload.usage.total_tokens,
      metadata: {},
    };

    set((s) => ({
      messages: {
        ...s.messages,
        [cid]: [...(s.messages[cid] || []), assistantMsg],
      },
      streamingText: null,
      streamingMsgId: null,
    }));
  },

  handleStreamError: (payload: GenErrorPayload) => {
    const { streamingText, streamingMsgId } = get();
    const cid = payload.conversation_id;
    // 保留已生成的部分文本为助手消息，避免流式中途报错时已生成内容凭空消失
    if (streamingMsgId && streamingText && streamingText.length > 0) {
      const assistantMsg: Message = {
        id: streamingMsgId,
        conversation_id: cid,
        role: "assistant",
        content: [{ type: "text", text: streamingText }],
        created_at: new Date().toISOString(),
        token_count: null,
        metadata: {},
      };
      set((s) => ({
        messages: { ...s.messages, [cid]: [...(s.messages[cid] || []), assistantMsg] },
        streamingText: null,
        streamingMsgId: null,
        error: payload.error,
      }));
    } else {
      set({ streamingText: null, streamingMsgId: null, error: payload.error });
    }
  },

  clearStreaming: () => {
    set({ streamingText: null, streamingMsgId: null });
  },

  regenerate: async (messageId: string, conversationId?: string) => {
    const state = get();
    const cid = conversationId || state.activeId;
    if (!cid) { set({ error: "No active conversation" }); return; }
    if (state.streamingText !== null) return;

    // 本地截断 messageId 及其后的消息（与后端 delete_from 对齐）。
    // 否则流式完成后新回复 append 到旧回复后面，出现 [user, 旧回复, 新回复] 重复。
    set((s) => {
      const msgs = s.messages[cid] || [];
      const idx = msgs.findIndex((m) => m.id === messageId);
      if (idx < 0) return {};
      return { messages: { ...s.messages, [cid]: msgs.slice(0, idx) } };
    });

    // 先标记流式开始，使首块竞态期间能锁存 message_id
    set({ streamingText: "", streamingMsgId: null });
    const s = useSettingsStore.getState();
    try {
      const msgId = await chatService.regenerate({
        conversationId: cid,
        messageId,
        apiKey: s.apiKey,
        apiBaseUrl: s.apiBaseUrl,
        model: s.defaultModel,
        agentMode: state.agentMode,
      });
      set((st) => ({ streamingMsgId: msgId, streamingText: st.streamingText ?? "" }));
    } catch (err) {
      set({ error: String(err), streamingText: null, streamingMsgId: null });
    }
  },

  updateMessage: async (messageId: string, content: string) => {
    try {
      await chatService.updateMsg(messageId, content);
      // 更新本地消息内容
      set((s) => {
        const updated = { ...s.messages };
        for (const [cid, msgs] of Object.entries(updated)) {
          const idx = msgs.findIndex((m) => m.id === messageId);
          if (idx >= 0) {
            const msg = { ...msgs[idx] };
            msg.content = [{ type: "text" as const, text: content }];
            const newMsgs = [...msgs];
            newMsgs[idx] = msg;
            updated[cid] = newMsgs;
            break;
          }
        }
        return { messages: updated };
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  deleteMessage: async (messageId: string, conversationId?: string) => {
    const state = get();
    const cid = conversationId || state.activeId;
    if (!cid) { set({ error: "No active conversation" }); return; }
    try {
      await chatService.deleteMsgFrom(cid, messageId);
      // 重新加载该对话消息
      const msgs = await messageService.list(cid);
      set((s) => ({
        messages: { ...s.messages, [cid]: msgs },
        streamingText: null,
        streamingMsgId: null,
      }));
      // 不强制设 activeId：异步删除+重载期间用户可能已切到别的对话，拽回是反直觉的。
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setError: (msg: string) => set({ error: msg }),
  clearError: () => set({ error: null }),

  toggleAgentMode: () => set((s) => ({ agentMode: !s.agentMode })),

  addToolEvent: (conversationId: string, event: ToolCallEvent) => {
    set((s) => {
      const existing = s.toolEvents[conversationId] || [];
      return { toolEvents: { ...s.toolEvents, [conversationId]: [...existing, event] } };
    });
  },
}));
