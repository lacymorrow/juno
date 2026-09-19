import { renderHook } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useBackendEvents } from "@/hooks/useBackendEvents";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve({ backend_running: true, desktop_available: true })),
}));

vi.mock("@/lib/ttsService", () => ({
  stopTTS: vi.fn(() => Promise.resolve()),
}));

// Capture every handler the hook registers, keyed by event name, so a test
// can fire a backend event directly.
const handlers = new Map<string, (payload: unknown) => void>();
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

function renderBackendEvents() {
  const setIsProcessing = vi.fn();
  const setConversationWithPruning = vi.fn();
  renderHook(() =>
    useBackendEvents({
      addSystemMessage: vi.fn(),
      addAssistantMessage: vi.fn(),
      setConversationWithPruning,
      setIsProcessing,
      setServerStatus: vi.fn(),
    }),
  );
  return { setIsProcessing, setConversationWithPruning };
}

describe("useBackendEvents: unified submission state", () => {
  beforeEach(() => {
    handlers.clear();
  });

  it("appends the user message and switches processing on when the backend announces a query", () => {
    const { setIsProcessing, setConversationWithPruning } = renderBackendEvents();

    handlers.get("user-message-submitted")?.({
      content: "Play my liked songs on Spotify",
      timestamp: 1234,
    });

    expect(setIsProcessing).toHaveBeenCalledWith(true);
    const updater = setConversationWithPruning.mock.calls[0][0] as (prev: unknown[]) => unknown[];
    expect(updater([])).toEqual([
      { role: "user", content: "Play my liked songs on Spotify", timestamp: 1234 },
    ]);
  });

  it("switches processing off when the backend reports the run finished", () => {
    const { setIsProcessing } = renderBackendEvents();

    handlers.get("agent-active")?.(false);
    expect(setIsProcessing).toHaveBeenCalledWith(false);

    setIsProcessing.mockClear();
    handlers.get("agent-active")?.(true);
    expect(setIsProcessing).not.toHaveBeenCalled();
  });

  it("does not settle when the microphone closes: capture is not the run", () => {
    const { setIsProcessing } = renderBackendEvents();

    // Capture ends the moment the person stops speaking, which is before the
    // run it started has done anything. While both phases were announced as
    // `agent-active`, this false switched the chat surface out of "working"
    // and the agent went on working with nothing on screen to say so. The hook
    // wants the run, so it must not listen to the microphone at all.
    expect(handlers.has("agent-capture-active")).toBe(false);

    handlers.get("agent-capture-active")?.(false);
    expect(setIsProcessing).not.toHaveBeenCalled();
  });

  it("switches processing off on an agent error", () => {
    const { setIsProcessing } = renderBackendEvents();

    handlers.get("agent-error")?.({
      agent_state: "Failed",
      error_message: "Rate limit exceeded",
      original_query: "hello",
    });

    expect(setIsProcessing).toHaveBeenCalledWith(false);
  });
});
