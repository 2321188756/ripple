import { describe, expect, it } from "vitest";
import type { GenCompletePayload, StreamChunkPayload, ToolCallEvent } from "@/types";

const streamChunk: StreamChunkPayload = {
  contract_version: 1,
  stream_id: "stream-1",
  conversation_id: "conversation-1",
  message_id: "message-1",
  seq: 1,
  delta_text: "hello",
  finish_reason: null,
};

const toolEvent: ToolCallEvent = {
  contract_version: 1,
  stream_id: "stream-1",
  conversation_id: "conversation-1",
  message_id: "message-1",
  seq: 2,
  tool_call_id: "tool-1",
  tool_name: "calculator",
  tool_input: { expression: "1+1" },
  tool_output: "2",
  status: "success",
};

const complete: GenCompletePayload = {
  contract_version: 1,
  stream_id: "stream-1",
  conversation_id: "conversation-1",
  message_id: "message-1",
  seq: 3,
  outcome: "completed",
  finish_reason: "stop",
  usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
};

describe("stream event contract fixtures", () => {
  it("keeps correlation and sequence fields on every event", () => {
    for (const event of [streamChunk, toolEvent, complete]) {
      expect(event.contract_version).toBe(1);
      expect(event.stream_id).toBe("stream-1");
      expect(event.conversation_id).toBe("conversation-1");
      expect(event.message_id).toBe("message-1");
      expect(event.seq).toBeGreaterThan(0);
    }
  });

  it("uses one text delta field and structured tool data", () => {
    expect(streamChunk.delta_text).toBe("hello");
    expect(toolEvent.tool_input).toEqual({ expression: "1+1" });
    expect(toolEvent.tool_output).toBe("2");
  });
});
