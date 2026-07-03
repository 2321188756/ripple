import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "@/stores/chatStore";
import type {
  StreamChunkPayload,
  GenCompletePayload,
  GenErrorPayload,
  ToolCallEvent,
} from "@/types";

/**
 * 注册全局 Tauri 流式事件监听。
 * 在 App 顶层调用一次即可，负责把后端事件转发到 chatStore。
 */
export function useStreamEvents() {
  useEffect(() => {
    const un1 = listen<StreamChunkPayload>("chat:stream-chunk", (e) =>
      useChatStore.getState().appendToStreaming(e.payload),
    );
    const un2 = listen<GenCompletePayload>("chat:gen-complete", (e) =>
      useChatStore.getState().finalizeStreaming(e.payload),
    );
    const un3 = listen<GenErrorPayload>("chat:gen-error", (e) =>
      useChatStore.getState().handleStreamError(e.payload),
    );
    const un4 = listen<ToolCallEvent>("chat:tool-call", (e) => {
      const store = useChatStore.getState();
      if (store.activeId) store.addToolEvent(store.activeId, e.payload);
    });

    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
    };
  }, []);
}
