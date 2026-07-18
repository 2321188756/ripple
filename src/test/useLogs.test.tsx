import { StrictMode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useLogs } from "@/hooks/useLogs";
import { logService, type LogSnapshot } from "@/services/log.service";

vi.mock("@/services/log.service", () => ({
  logService: { getLogs: vi.fn() },
}));

const snapshot: LogSnapshot = {
  path: "D:/tmp/ripple.log",
  fileSize: 42,
  modifiedAtMs: 1,
  byteCap: 512 * 1024,
  requestedLines: 500,
  returnedLines: 1,
  truncated: false,
  entries: [{
    timestamp: "2026-07-15T00:00:00Z",
    level: "info",
    target: "test",
    message: "safe diagnostic",
    raw: "2026-07-15T00:00:00Z INFO test: safe diagnostic",
  }],
};

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <StrictMode>{children}</StrictMode>
);

describe("useLogs", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("accepts a log snapshot after StrictMode remounts the effect", async () => {
    vi.mocked(logService.getLogs).mockResolvedValue(snapshot);
    const { result } = renderHook(() => useLogs(true, 60_000), { wrapper });

    await waitFor(() => expect(result.current.snapshot.entries).toEqual(snapshot.entries));
    expect(result.current.error).toBeNull();
  });

  it("ignores a response that resolves after unmount", async () => {
    let resolve!: (value: LogSnapshot) => void;
    vi.mocked(logService.getLogs).mockReturnValue(new Promise((done) => { resolve = done; }));
    const { result, unmount } = renderHook(() => useLogs(true, 60_000), { wrapper });

    unmount();
    await act(async () => { resolve(snapshot); });

    expect(result.current.snapshot.entries).toEqual([]);
  });
});
