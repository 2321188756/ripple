import { create } from "zustand";
import { conversationService, messageService, chatService, logService } from "@/services";
import { invoke } from "@/services/invoke";
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
  ApprovalRequestEvent,
} from "@/types";

// ---------- Store ----------

/** 缓存最近一次发送/重生成请求，供错误横幅"重试"使用 */
type LastRequest =
  | { type: "send"; content: string; images?: string[]; userMsgId: string; conversationId: string }
  | { type: "regenerate"; messageId: string; conversationId: string }
  | null;

interface MessagePagingState {
  hasOlder: boolean;
  loadingOlder: boolean;
  error: string | null;
}

interface ChatState {
  conversations: Conversation[];
  activeId: string | null;
  messages: Record<string, Message[]>;
  messagePaging: Record<string, MessagePagingState>;
  loadingConversationId: string | null;
  agentMode: boolean;
  streamingText: string | null;
  streamingMsgId: string | null;
  streamingStreamId: string | null;
  streamingConversationId: string | null;
  streamingSeq: number;
  streamingBlocks: ContentBlock[];
  loading: boolean;
  error: string | null;
  /** 记住每个 Agent 上次活跃的会话 */
  lastActivePerAgent: Record<string, string>;
  /** 最近一次请求（send/regenerate），供错误重试 */
  lastRequest: LastRequest;
  /** 待审批的工具调用请求（后端 emit chat:tool-approval-request，前端弹框确认） */
  pendingApprovals: ApprovalRequestEvent[];

  setError: (msg: string) => void;
  clearError: () => void;
  addApprovalRequest: (req: ApprovalRequestEvent) => void;
  resolveApproval: (requestId: string, approved: boolean, trustTool: boolean) => Promise<void>;
  toggleAgentMode: () => void;

  // actions
  loadConversations: (agentId?: string) => Promise<void>;
  createConversation: (agentId?: string) => Promise<string>;
  switchConversation: (id: string, agentId?: string) => Promise<void>;
  loadOlderMessages: (conversationId?: string) => Promise<void>;
  /** 选中 Agent 后恢复其上次活跃会话，没有则选最新 */
  restoreLastActive: (agentId: string) => Promise<void>;
  sendMessage: (content: string, images?: string[]) => Promise<void>;
  stopGeneration: () => Promise<void>;
  /** 重生成：从指定消息重新生成 */
  regenerate: (messageId: string, conversationId?: string) => Promise<void>;
  /** 重试最近一次失败的 send/regenerate */
  retry: () => Promise<void>;
  /** 更新消息内容（编辑后自动后续需手动 regenerate） */
  updateMessage: (messageId: string, content: string) => Promise<void>;
  /** 删除消息及其后所有消息 */
  deleteMessage: (messageId: string, conversationId?: string) => Promise<void>;
  appendToStreaming: (chunk: StreamChunkPayload) => void;
  appendToolEvent: (event: ToolCallEvent) => void;
  finalizeStreaming: (payload: GenCompletePayload) => void;
  /** 流式出错：保留已生成部分为助手消息，清流并设置错误 */
  handleStreamError: (payload: GenErrorPayload) => void;
  clearStreaming: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  activeId: null,
  messages: {},
  messagePaging: {},
  loadingConversationId: null,
  agentMode: false,
  streamingText: null,
  streamingMsgId: null,
  streamingStreamId: null,
  streamingConversationId: null,
  streamingSeq: 0,
  streamingBlocks: [],
  loading: false,
  error: null,
  lastActivePerAgent: {},
  lastRequest: null,
  pendingApprovals: [],

  loadConversations: async (agentId?: string) => {
    try {
      const convos = await conversationService.list(agentId ? { agentId } : {});
      set({ conversations: convos });
    } catch (error) {
      set({ error: `Failed to load conversations: ${String(error)}` });
    }
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

    set({ loadingConversationId: id });
    try {
      const msgs = await messageService.list(id);
      if (get().loadingConversationId !== id) return;
      set((s) => ({
        activeId: id,
        loadingConversationId: null,
        streamingText: null,
        streamingMsgId: null,
        streamingStreamId: null,
        streamingConversationId: null,
        streamingSeq: 0,
        streamingBlocks: [],
        messages: { ...s.messages, [id]: msgs },
        messagePaging: {
          ...s.messagePaging,
          [id]: { hasOlder: msgs.length === 50, loadingOlder: false, error: null },
        },
      }));
    } catch (error) {
      if (get().loadingConversationId === id) {
        set({ loadingConversationId: null, error: `Failed to load messages: ${String(error)}` });
      }
    }
  },

  loadOlderMessages: async (conversationId?: string) => {
    const cid = conversationId ?? get().activeId;
    if (!cid) return;
    const state = get();
    const paging = state.messagePaging[cid];
    const existing = state.messages[cid] ?? [];
    if (paging?.loadingOlder || paging?.hasOlder === false || existing.length === 0) return;
    set((s) => ({
      messagePaging: {
        ...s.messagePaging,
        [cid]: { hasOlder: true, loadingOlder: true, error: null },
      },
    }));
    try {
      const page = await messageService.list(cid, { limit: 50, beforeId: existing[0].id });
      set((s) => {
        const current = s.messages[cid] ?? [];
        const seen = new Set(current.map((message) => message.id));
        const prepend = page.filter((message) => !seen.has(message.id));
        return {
          messages: { ...s.messages, [cid]: [...prepend, ...current] },
          messagePaging: {
            ...s.messagePaging,
            [cid]: { hasOlder: page.length === 50, loadingOlder: false, error: null },
          },
        };
      });
    } catch (error) {
      set((s) => ({
        messagePaging: {
          ...s.messagePaging,
          [cid]: { hasOlder: true, loadingOlder: false, error: String(error) },
        },
        error: `Failed to load older messages: ${String(error)}`,
      }));
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
    } else {
      // 该 Agent 暂无会话：清空 activeId，右侧显示空状态。
      // 否则会残留上一个 Agent 的对话，看着像"切 Agent 没生效"。
      set({ activeId: null, streamingText: null, streamingMsgId: null });
    }
  },

  sendMessage: async (content: string, images?: string[]) => {
    const state = get();
    if (state.streamingText !== null) {
      await logService.log({ event: "chat_send_ignored" });
      return;
    }
    let aid = state.activeId;
    if (!aid) {
      await logService.log({ event: "chat_conversation_creating" });
      try {
        // 用当前选中 Agent 建会话，使会话归属该 Agent（元数据带 agent_id），切回时能恢复
        const agentId = useAgentStore.getState().selectedAgent?.id;
        const id = await state.createConversation(agentId);
        await state.switchConversation(id, agentId);
        aid = id;
      } catch {
        await logService.log({ event: "chat_conversation_create_failed" });
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
      // 缓存请求供错误重试
      lastRequest: { type: "send", content, images: images && images.length > 0 ? images : undefined, userMsgId: userMsg.id, conversationId: aid },
    }));

    await logService.log({ event: "chat_send_started", conversationId: aid, contentChars: content.length });
    // 先标记流式开始（streamingText="" 非 null），使 await 期间到达的首块能被
    // appendToStreaming 锁存 message_id，避免快模型/本地模型开头丢字。
    set({ streamingText: "", streamingMsgId: null, streamingStreamId: null, streamingConversationId: aid, streamingSeq: 0, streamingBlocks: [] });
    try {
      const s = useSettingsStore.getState();
      const agentMode = get().agentMode;
      const agent = useAgentStore.getState().selectedAgent;
      const msgId = await chatService.send({
        conversationId: aid,
        content,
        images: images && images.length > 0 ? images : undefined,
        apiBaseUrl: s.apiBaseUrl,
        model: s.defaultModel,
        agentMode,
        temperature: agent?.temperature,
        maxTokens: agent?.max_tokens,
        topP: agent?.top_p,
        userMessageId: userMsg.id,
      });
      await logService.log({ event: "chat_send_succeeded", messageId: msgId });
      // 补设 msgId；若首块已锁存则同值，且不覆盖已累积的 streamingText
      set((st) => ({ streamingMsgId: msgId, streamingText: st.streamingText ?? "" }));
    } catch (err) {
      const msg = typeof err === "string" ? err : "send_message failed";
      await logService.log({ event: "chat_send_failed" });
      set({ error: msg, streamingText: null, streamingMsgId: null });
    }
  },

  stopGeneration: async () => {
    const { activeId } = get();
    if (!activeId) return;
    try {
      await chatService.stop(activeId);
    } catch (error) {
      set({ error: `Failed to stop generation: ${String(error)}` });
    }
  },

  appendToStreaming: (chunk: StreamChunkPayload) => {
    if (chunk.contract_version !== 1) return;
    const s = get();
    if (s.streamingText === null) return;
    if (s.streamingConversationId && s.streamingConversationId !== chunk.conversation_id) return;
    if (s.streamingStreamId && s.streamingStreamId !== chunk.stream_id) return;
    if (s.streamingMsgId && s.streamingMsgId !== chunk.message_id) return;
    if (s.streamingStreamId === chunk.stream_id && chunk.seq <= s.streamingSeq) return;

    set({
      streamingStreamId: chunk.stream_id,
      streamingConversationId: chunk.conversation_id,
      streamingMsgId: chunk.message_id,
      streamingSeq: chunk.seq,
      streamingText: chunk.delta_text ? s.streamingText + chunk.delta_text : s.streamingText,
    });
  },

  appendToolEvent: (event: ToolCallEvent) => {
    if (event.contract_version !== 1) return;
    const s = get();
    if (s.streamingText === null) return;
    if (s.streamingConversationId && s.streamingConversationId !== event.conversation_id) return;
    if (s.streamingStreamId && s.streamingStreamId !== event.stream_id) return;
    if (s.streamingMsgId && s.streamingMsgId !== event.message_id) return;
    if (s.streamingStreamId === event.stream_id && event.seq <= s.streamingSeq) return;
    const toolCall: ContentBlock = {
      type: "tool_call",
      id: event.tool_call_id,
      name: event.tool_name,
      arguments: event.tool_input,
    };
    const toolResult: ContentBlock = {
      type: "tool_result",
      tool_call_id: event.tool_call_id,
      content: event.tool_output,
    };
    const withoutCall = s.streamingBlocks.filter((block) =>
      !(block.type === "tool_call" && block.id === event.tool_call_id) &&
      !(block.type === "tool_result" && block.tool_call_id === event.tool_call_id),
    );
    set({
      streamingStreamId: event.stream_id,
      streamingConversationId: event.conversation_id,
      streamingMsgId: event.message_id,
      streamingSeq: event.seq,
      streamingBlocks: [...withoutCall, toolCall, toolResult],
    });
  },

  finalizeStreaming: (payload: GenCompletePayload) => {
    const { streamingText, streamingMsgId, streamingStreamId, streamingConversationId, streamingSeq, streamingBlocks } = get();
    if (!streamingMsgId || streamingText === null) return;
    if (payload.contract_version !== 1 || payload.stream_id !== streamingStreamId ||
        payload.message_id !== streamingMsgId || payload.conversation_id !== streamingConversationId ||
        payload.seq <= streamingSeq) return;
    const cid = payload.conversation_id;

    const assistantMsg: Message = {
      id: streamingMsgId,
      conversation_id: cid,
      role: "assistant",
      content: [
        ...(streamingText ? [{ type: "text" as const, text: streamingText }] : []),
        ...streamingBlocks,
      ],
      created_at: new Date().toISOString(),
      token_count: payload.usage.total_tokens,
      metadata: { completion_state: payload.outcome, finish_reason: payload.finish_reason },
    };

    set((s) => ({
      messages: {
        ...s.messages,
        [cid]: [...(s.messages[cid] || []), assistantMsg],
      },
      streamingText: null,
      streamingMsgId: null,
      streamingStreamId: null,
      streamingConversationId: null,
      streamingSeq: 0,
      streamingBlocks: [],
    }));
  },

  handleStreamError: (payload: GenErrorPayload) => {
    const { streamingText, streamingMsgId, streamingStreamId, streamingConversationId, streamingSeq, streamingBlocks } = get();
    if (streamingText === null) return;
    if (payload.contract_version !== 1 ||
        (streamingStreamId !== null && payload.stream_id !== streamingStreamId) ||
        (streamingMsgId !== null && payload.message_id !== streamingMsgId) ||
        (streamingConversationId !== null && payload.conversation_id !== streamingConversationId) ||
        payload.seq < streamingSeq) return;
    const cid = payload.conversation_id;
    // 保留已生成的部分文本为助手消息，避免流式中途报错时已生成内容凭空消失
    if (streamingMsgId && streamingText && streamingText.length > 0) {
      const assistantMsg: Message = {
        id: streamingMsgId,
        conversation_id: cid,
        role: "assistant",
        content: [
          ...(streamingText ? [{ type: "text" as const, text: streamingText }] : []),
          ...streamingBlocks,
        ],
        created_at: new Date().toISOString(),
        token_count: null,
        metadata: { completion_state: "failed" },
      };
      set((s) => ({
        messages: { ...s.messages, [cid]: [...(s.messages[cid] || []), assistantMsg] },
        streamingText: null,
        streamingMsgId: null,
        streamingStreamId: null,
        streamingConversationId: null,
        streamingSeq: 0,
        streamingBlocks: [],
        error: payload.error,
      }));
    } else {
      set({
        streamingText: null,
        streamingMsgId: null,
        streamingStreamId: null,
        streamingConversationId: null,
        streamingSeq: 0,
        streamingBlocks: [],
        error: payload.error,
      });
    }
  },

  clearStreaming: () => {
    set({
      streamingText: null,
      streamingMsgId: null,
      streamingStreamId: null,
      streamingConversationId: null,
      streamingSeq: 0,
      streamingBlocks: [],
    });
  },

  regenerate: async (messageId: string, conversationId?: string) => {
    const state = get();
    const cid = conversationId || state.activeId;
    if (!cid) { set({ error: "No active conversation" }); return; }
    if (state.streamingText !== null) return;

    const originalMessages = [...(state.messages[cid] || [])];
    // 本地先反映截断；后端失败时从权威存储重新加载，避免历史永久消失。
    set((s) => {
      const msgs = s.messages[cid] || [];
      const idx = msgs.findIndex((m) => m.id === messageId);
      if (idx < 0) return {};
      // user 消息保留本身（idx+1），assistant 消息删除本身（idx）
      const keep = msgs[idx]?.role === "user" ? idx + 1 : idx;
      return { messages: { ...s.messages, [cid]: msgs.slice(0, keep) } };
    });

    // 先标记流式开始，使首块竞态期间能锁存 message_id
    set({ streamingText: "", streamingMsgId: null, lastRequest: { type: "regenerate", messageId, conversationId: cid } });
    const s = useSettingsStore.getState();
    const agent = useAgentStore.getState().selectedAgent;
    try {
      const msgId = await chatService.regenerate({
        conversationId: cid,
        messageId,
        apiBaseUrl: s.apiBaseUrl,
        model: s.defaultModel,
        agentMode: state.agentMode,
        temperature: agent?.temperature,
        maxTokens: agent?.max_tokens,
        topP: agent?.top_p,
      });
      set((st) => ({ streamingMsgId: msgId, streamingText: st.streamingText ?? "" }));
    } catch (err) {
      try {
        const authoritative = await messageService.list(cid);
        set((current) => ({
          messages: { ...current.messages, [cid]: authoritative },
          error: String(err),
          streamingText: null,
          streamingMsgId: null,
          streamingStreamId: null,
          streamingConversationId: null,
          streamingSeq: 0,
          streamingBlocks: [],
        }));
      } catch {
        set((current) => ({
          messages: { ...current.messages, [cid]: originalMessages },
          error: String(err),
          streamingText: null,
          streamingMsgId: null,
          streamingStreamId: null,
          streamingConversationId: null,
          streamingSeq: 0,
          streamingBlocks: [],
        }));
      }
    }
  },

  retry: async () => {
    const req = get().lastRequest;
    if (!req) return;
    set({ error: null });
    if (req.type === "send") {
      // 删除失败的那次用户消息（及部分回复），再重发，避免重复用户消息
      await get().deleteMessage(req.userMsgId, req.conversationId);
      await get().sendMessage(req.content, req.images);
    } else {
      await get().regenerate(req.messageId, req.conversationId);
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

  addApprovalRequest: (req: ApprovalRequestEvent) => {
    set((s) => ({ pendingApprovals: [...s.pendingApprovals, req] }));
  },

  resolveApproval: async (requestId: string, approved: boolean, trustTool: boolean) => {
    // 先从队列移除（即时反馈），再通知后端（唤醒阻塞的 await）
    set((s) => ({ pendingApprovals: s.pendingApprovals.filter((p) => p.request_id !== requestId) }));
    try {
      await invoke("approve_tool_call", { requestId, approved, trustTool });
    } catch (e) {
      console.error("approve_tool_call failed:", e);
    }
  },
}));
