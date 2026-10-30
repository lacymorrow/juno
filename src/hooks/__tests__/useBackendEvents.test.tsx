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
  });

  it("switches processing back on when the next run starts", () => {
    const { setIsProcessing } = renderBackendEvents();

    // A query sent mid-run: the old run's end lands after the new query
    // switched processing on, so the new run's start has to switch it back.
    handlers.get("user-message-submitted")?.({ content: "do this instead", timestamp: 1 });
    handlers.get("agent-active")?.(false);
    handlers.get("agent-active")?.(true);
    expect(setIsProcessing).toHaveBeenLastCalledWith(true);
  });

  it("stays working when a stream ends before a tool call", () => {
    const { setIsProcessing } = renderBackendEvents();

    // A tool-using turn closes its text bubble before every tool call. That
    // is not the run ending, so the chat must not go idle while tools run.
    handlers.get("agent-stream-end")?.({ message_id: "m1", complete_text: "Opening Safari." });
    expect(setIsProcessing).not.toHaveBeenCalled();

    // A reply that carries its final state does end it.
    handlers.get("agent-stream-end")?.({
      message_id: "m2",
      complete_text: "No connection.",
      agent_state: "Finished",
    });
    expect(setIsProcessing).toHaveBeenCalledWith(false);
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

/**
 * The dead control this fixes.
 *
 * An approval that nobody answered inside the window was denied in Rust and
 * announced nowhere, so `approval_state` stayed "pending": the row kept its
 * Allow and Don't allow buttons and pressing either did nothing, because the
 * request they referred to no longer existed. Rust now emits
 * `tool-approval-resolved` for every outcome, and these pin the frontend half
 * shut so it cannot rot back.
 */
describe("useBackendEvents: an approval question always settles", () => {
  beforeEach(() => {
    handlers.clear();
  });

  const pendingRow = {
    role: "tool_call_request",
    content: "Run this in the terminal: npm install",
    tool_id: "batch-1",
    approval_state: "pending",
  };

  function resolve(payload: unknown) {
    const { setConversationWithPruning } = renderBackendEvents();
    handlers.get("tool-approval-resolved")?.(payload);
    const updater = setConversationWithPruning.mock.calls[0]?.[0] as
      | ((prev: unknown[]) => unknown[])
      | undefined;
    return updater;
  }

  it("is listening for the resolution at all", () => {
    renderBackendEvents();
    expect(handlers.has("tool-approval-resolved")).toBe(true);
  });

  it("settles a timed-out row to denied, so the buttons go away", () => {
    const updater = resolve({
      tool_id: "batch-1",
      resolution: "denied",
      reason: "nobody answered in time",
    });

    expect(updater?.([pendingRow])).toEqual([
      { ...pendingRow, approval_state: "denied" },
    ]);
  });

  it("settles an allowed row to approved", () => {
    const updater = resolve({ tool_id: "batch-1", resolution: "approved" });

    expect(updater?.([pendingRow])).toEqual([
      { ...pendingRow, approval_state: "approved" },
    ]);
  });

  it("leaves other rows and already-answered rows alone", () => {
    const answered = { ...pendingRow, tool_id: "batch-0", approval_state: "approved" };
    const other = { role: "user", content: "hi" };
    const updater = resolve({ tool_id: "batch-1", resolution: "denied" });

    // An "approved" row must not be rewritten to "denied" by a late
    // resolution, and nothing without this tool_id may be touched.
    expect(updater?.([answered, other, pendingRow])).toEqual([
      answered,
      other,
      { ...pendingRow, approval_state: "denied" },
    ]);
  });

  it("ignores a resolution with no tool_id instead of rewriting the transcript", () => {
    const { setConversationWithPruning } = renderBackendEvents();
    handlers.get("tool-approval-resolved")?.({ resolution: "denied" });
    expect(setConversationWithPruning).not.toHaveBeenCalled();
  });
});

/**
 * The staleness this fixes.
 *
 * `ax-grounding-audit` and the `tool_call_result` it belongs to are joined by
 * nothing but their order: the audit sits in a single-slot buffer until the
 * next `computer` result consumes it. End the run in between — Escape, an
 * error, stop-all — and the audit outlives the action it describes, so a later
 * unrelated click is labelled with the earlier element's role and label.
 */
describe("useBackendEvents: an unconsumed AX audit does not outlive its run", () => {
  beforeEach(() => {
    handlers.clear();
  });

  const audit = {
    action: "left_click",
    ax_grounded: true,
    ax_role: "AXButton",
    ax_label: "Send",
    screen_coordinate: [120, 240],
    timestamp: 1,
  };

  /** Fire a `computer` result and return the row it produced. */
  function computerResult(setConversationWithPruning: ReturnType<typeof vi.fn>) {
    handlers.get("agent-event")?.({
      type: "tool_call_result",
      payload: { tool_name: "computer", success: true, content: "Clicked." },
    });
    const updater = setConversationWithPruning.mock.calls.at(-1)?.[0] as (
      prev: unknown[],
    ) => Record<string, unknown>[];
    return updater([])[0];
  }

  // The control. Without it, every assertion below would also pass if the
  // buffer stopped attaching audits at all.
  it("still labels the result when the audit's own action completes", () => {
    const { setConversationWithPruning } = renderBackendEvents();

    handlers.get("ax-grounding-audit")?.(audit);

    expect(computerResult(setConversationWithPruning)).toMatchObject({
      ax_grounded: true,
      ax_role: "AXButton",
      ax_label: "Send",
      ax_action: "left_click",
      ax_screen_coordinate: [120, 240],
    });
  });

  const stops: [string, unknown][] = [
    ["agent-stopping", undefined],
    [
      "agent-error",
      { agent_state: "Failed", error_message: "cancelled", original_query: "click send" },
    ],
    ["agent-stop-all", undefined],
  ];

  for (const [event, payload] of stops) {
    it(`drops the audit on ${event}, so the next action is not mislabelled`, () => {
      const { setConversationWithPruning } = renderBackendEvents();

      handlers.get("ax-grounding-audit")?.(audit);
      handlers.get(event)?.(payload);

      const row = computerResult(setConversationWithPruning);
      expect(row).not.toHaveProperty("ax_role");
      expect(row).not.toHaveProperty("ax_label");
      expect(row).not.toHaveProperty("ax_grounded");
      expect(row).not.toHaveProperty("ax_action");
      expect(row).not.toHaveProperty("ax_screen_coordinate");
    });
  }
});
