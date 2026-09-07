import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FloatingBar, floatingBarWindowSize } from "../FloatingBar";

// ── Tauri + hook mocks ───────────────────────────────────────────────

const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]) => Promise.resolve()),
  // Bar-state events arrive through `listen` directly; conversation events
  // arrive through useEventListener. Both are captured by event name so a
  // test can play the backend. Hoisted: module-level services call listen()
  // at import time, before this file's own consts would initialise.
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  resizeWindowIfChanged: vi.fn((_size: { width: number; height: number }) => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    listenHandlers.set(event, handler);
    return () => listenHandlers.delete(event);
  }),
}));

vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    eventHandlers.set(event, handler);
  },
}));

// The window-level focus callback is captured so a test can simulate the
// OS making the bar window key (a click from another app).
const windowFocus = vi.hoisted(() => ({
  handler: undefined as undefined | ((event: { payload: boolean }) => void),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    onFocusChanged: vi.fn(async (handler: (event: { payload: boolean }) => void) => {
      windowFocus.handler = handler;
      return () => {};
    }),
    startDragging: vi.fn(async () => {}),
  }),
}));

const webviewSetFocus = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ setFocus: webviewSetFocus }),
}));

vi.mock("@/hooks/useWindowSize", () => ({
  useWindowSize: () => ({ resizeWindowIfChanged }),
}));

vi.mock("@/hooks/useAgentSessions", () => ({
  useAgentSessions: () => ({ sessions: [], focusSession: vi.fn(), cancelSession: vi.fn() }),
}));

vi.mock("@/lib/ttsService", () => ({ stopTTS: vi.fn(() => Promise.resolve()) }));

// ── Helpers ──────────────────────────────────────────────────────────

const barState = (overrides: Record<string, unknown> = {}) => ({
  barState: "default",
  inputValue: "",
  lastSubmittedValue: "",
  currentError: null,
  transcriptionText: "",
  spokenText: "",
  voiceMode: "idle",
  audioLevel: 0,
  isAgentWorking: false,
  isDictationMode: false,
  isAlwaysListening: false,
  agentState: null,
  ...overrides,
});

const setBarState = (overrides: Record<string, unknown>) =>
  act(() => {
    listenHandlers.get("bar-state-update")?.({ payload: barState(overrides) });
  });

const fire = (event: string, payload: unknown) =>
  act(() => {
    eventHandlers.get(event)?.(payload);
  });

const submitUserMessage = (content: string) =>
  fire("user-message-submitted", { content, timestamp: 1_700_000_000_000 });

const streamAssistant = (id: string, text: string) => {
  fire("agent-stream-start", { message_id: id });
  fire("agent-text-stream", { chunk: text, message_id: id });
  fire("agent-stream-end", { message_id: id, complete_text: text, agent_state: "Finished" });
};

async function renderBar() {
  const utils = render(<FloatingBar />);
  // Let the async listen() registrations settle.
  await act(async () => {});
  return utils;
}

const lastResize = () =>
  resizeWindowIfChanged.mock.calls[resizeWindowIfChanged.mock.calls.length - 1]?.[0];

beforeEach(() => {
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  listenHandlers.clear();
  eventHandlers.clear();
});

// ── Tests ────────────────────────────────────────────────────────────

describe("floatingBarWindowSize", () => {
  it("is a fixed-width window that only grows downward", () => {
    expect(floatingBarWindowSize({ paneOpen: false, rosterVisible: false })).toEqual({
      width: 467,
      height: 92,
    });
    expect(floatingBarWindowSize({ paneOpen: true, rosterVisible: false })).toEqual({
      width: 467,
      height: 460,
    });
    expect(floatingBarWindowSize({ paneOpen: true, rosterVisible: true })).toEqual({
      width: 467,
      height: 494,
    });
  });
});

describe("FloatingBar", () => {
  it("starts as a compact pill with no conversation pane", async () => {
    await renderBar();

    expect(screen.getByTestId("floating-bar")).toHaveAttribute("data-state", "default");
    // One click away: the idle pill already is the input.
    expect(screen.getByPlaceholderText("Ask Juno")).toBeInTheDocument();
    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
    expect(lastResize()).toEqual({ width: 467, height: 92 });
  });

  it("opens the chat pane with the user's message when the backend announces a query", async () => {
    await renderBar();

    submitUserMessage("Play my liked songs on Spotify");

    const pane = screen.getByTestId("bar-chat-pane");
    expect(pane).toBeInTheDocument();
    expect(pane).toHaveClass("dark");
    expect(screen.getByText("Play my liked songs on Spotify")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("working");
    expect(lastResize()).toEqual({ width: 467, height: 460 });
  });

  it("streams the assistant response into the pane and settles when the agent goes idle", async () => {
    await renderBar();

    submitUserMessage("What time is it?");
    streamAssistant("m1", "It is half past nine.");
    fire("agent-active", false);

    expect(screen.getByText("It is half past nine.")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("esc to close");
  });

  it("keeps the pill as an input for follow-ups while the pane is open", async () => {
    await renderBar();

    submitUserMessage("Hello");
    streamAssistant("m1", "Hi there.");
    fire("agent-active", false);
    setBarState({ barState: "default" });

    expect(screen.getByPlaceholderText("Follow up…")).toBeInTheDocument();
  });

  it("shows status and a Stop control instead of the input while the agent works", async () => {
    await renderBar();

    submitUserMessage("Open Safari");
    setBarState({ barState: "loading", agentState: "working", isAgentWorking: true });

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("working");

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(invoke).toHaveBeenCalledWith("stop_all_operations");
  });

  it("dismisses the pane on Escape only when nothing is running", async () => {
    await renderBar();

    submitUserMessage("Hello");
    // Still processing: Escape belongs to the Rust stop-key monitor.
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();

    streamAssistant("m1", "Hi.");
    fire("agent-active", false);
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
    expect(lastResize()).toEqual({ width: 467, height: 92 });
  });

  it("reopens a dismissed pane when the next query arrives", async () => {
    await renderBar();

    submitUserMessage("First");
    streamAssistant("m1", "One.");
    fire("agent-active", false);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss conversation" }));
    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();

    submitUserMessage("Second");

    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();
    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.getByText("Second")).toBeInTheDocument();
  });

  it("clears the conversation and closes the pane on New chat", async () => {
    await renderBar();

    submitUserMessage("Hello");
    streamAssistant("m1", "Hi.");
    fire("agent-active", false);
    fireEvent.click(screen.getByRole("button", { name: "New chat" }));

    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();

    submitUserMessage("Again");
    expect(screen.queryByText("Hello")).not.toBeInTheDocument();
    expect(screen.getByText("Again")).toBeInTheDocument();
  });

  it("submits typed input through the standard bar interaction and clears it", async () => {
    await renderBar();

    setBarState({ barState: "input" });
    const input = screen.getByPlaceholderText("Ask Juno");
    fireEvent.change(input, { target: { value: "  open my calendar " } });
    fireEvent.submit(input.closest("form")!);
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        elementId: "floating-bar",
        interaction: expect.objectContaining({
          interaction_type: "submit",
          data: { value: "open my calendar" },
        }),
      }),
    );
    expect(input).toHaveValue("");
  });

  it("tells the backend to expand when the idle input gains focus", async () => {
    await renderBar();

    fireEvent.focus(screen.getByPlaceholderText("Ask Juno"));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({
          interaction_type: "focus",
          data: { isFocused: true },
        }),
      }),
    );
  });

  it("focuses the input the moment the window becomes key, so one click is enough", async () => {
    await renderBar();
    const input = screen.getByPlaceholderText("Ask Juno");
    expect(input).not.toHaveFocus();

    act(() => windowFocus.handler?.({ payload: true }));
    await act(async () => {});

    expect(input).toHaveFocus();
    // Keystrokes only reach the page once the webview is first responder.
    expect(webviewSetFocus).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: "focus" }),
      }),
    );
  });

  it("shows the dictation-ready label instead of an input, and activates on click", async () => {
    await renderBar();

    setBarState({ barState: "dictation_ready" });
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByText("dictation ready")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Activate assistant" }));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: "click" }),
      }),
    );
  });

  it("keeps the input usable through the backend's expanding transition", async () => {
    await renderBar();

    setBarState({ barState: "expanding" });
    const input = screen.getByPlaceholderText("Ask Juno");
    expect(input).toBeEnabled();
    fireEvent.change(input, { target: { value: "hel" } });
    expect(input).toHaveValue("hel");
  });
});
