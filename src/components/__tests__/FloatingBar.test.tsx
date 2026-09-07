import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  BLUR_SETTLE_MS,
  FloatingBar,
  LEAVE_VERIFY_MS,
  SHRINK_DELAY_MS,
  floatingBarWindowSize,
  pickLayout,
  pointInRect,
} from "../FloatingBar";

// ── Tauri + hook mocks ───────────────────────────────────────────────

const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]) => Promise.resolve()),
  // Bar-state and hover events arrive through `listen` directly; conversation
  // events arrive through useEventListener. Both are captured by event name
  // so a test can play the backend. Hoisted: module-level services call
  // listen() at import time, before this file's own consts would initialise.
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  resizeWindowIfChanged: vi.fn(
    (_size: { width: number; height: number; anchorY?: number }) => Promise.resolve(),
  ),
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
const windowSetFocus = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const startDragging = vi.hoisted(() => vi.fn(() => Promise.resolve()));
// Where the OS says the cursor is; the window sits at 100,100 sized 164x66.
const cursor = vi.hoisted(() => ({ x: 0, y: 0 }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    onFocusChanged: vi.fn(async (handler: (event: { payload: boolean }) => void) => {
      windowFocus.handler = handler;
      return () => {};
    }),
    setFocus: windowSetFocus,
    startDragging,
    outerPosition: async () => ({ x: 100, y: 100 }),
    outerSize: async () => ({ width: 164, height: 66 }),
  }),
  cursorPosition: async () => ({ x: cursor.x, y: cursor.y }),
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

/**
 * The native tracking area reporting the mouse crossing the window's edge.
 * A leave is only believed once the cursor is confirmed outside the window,
 * so leaving also parks the cursor outside and lets that check run.
 */
const hover = async (inside: boolean) => {
  cursor.x = inside ? 150 : 0;
  cursor.y = inside ? 130 : 0;
  act(() => {
    listenHandlers.get(inside ? "mouse-entered-window" : "mouse-left-window")?.({
      payload: null,
    });
  });
  if (!inside) await settle(LEAVE_VERIFY_MS);
};

/** The native tracking area forwarding the cursor position, then a rAF flush. */
const move = async (x: number, y: number) => {
  act(() => {
    listenHandlers.get("mouse-moved-window")?.({ payload: { x, y } });
  });
  await act(async () => {
    await new Promise((r) => requestAnimationFrame(() => r(null)));
  });
};

/** Let a timer of `ms` fire, under real or fake timers. */
const settle = async (ms: number) => {
  if (vi.isFakeTimers()) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ms + 1);
    });
  } else {
    await act(async () => {
      await new Promise((r) => setTimeout(r, ms + 10));
    });
  }
};

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

const bar = () => screen.getByTestId("floating-bar");
const lastResize = () =>
  resizeWindowIfChanged.mock.calls[resizeWindowIfChanged.mock.calls.length - 1]?.[0];

/** Hover the pill and open the text input via its button. */
async function openInput() {
  await hover(true);
  fireEvent.click(screen.getByRole("button", { name: "Type to Juno" }));
  await act(async () => {});
  return screen.getByPlaceholderText("Ask Juno");
}

const interaction = (type: string, data?: unknown) =>
  expect.objectContaining({
    elementId: "floating-bar",
    interaction: expect.objectContaining(
      data === undefined ? { interaction_type: type } : { interaction_type: type, data },
    ),
  });

beforeEach(() => {
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  cursor.x = 0;
  cursor.y = 0;
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  windowSetFocus.mockClear();
  webviewSetFocus.mockClear();
  startDragging.mockClear();
  listenHandlers.clear();
  eventHandlers.clear();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

// ── Tests ────────────────────────────────────────────────────────────

describe("floatingBarWindowSize", () => {
  it("is exactly the pill plus shadow padding, anchored on the pill's centre", () => {
    expect(floatingBarWindowSize({ layout: "compact", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 88, height: 48, anchorY: 24 });
    expect(floatingBarWindowSize({ layout: "hover", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 164, height: 66, anchorY: 33 });
    expect(floatingBarWindowSize({ layout: "voice", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 252, height: 66, anchorY: 33 });
    expect(floatingBarWindowSize({ layout: "full", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 467, height: 92, anchorY: 46 });
    expect(floatingBarWindowSize({ layout: "full", paneOpen: true, rosterVisible: false }))
      .toEqual({ width: 467, height: 460, anchorY: 46 });
    expect(floatingBarWindowSize({ layout: "full", paneOpen: true, rosterVisible: true }))
      .toEqual({ width: 467, height: 494, anchorY: 46 });
  });
});

describe("pointInRect", () => {
  const r = { left: 10, top: 20, right: 50, bottom: 40 };
  it("is inclusive of the edges and rejects points outside", () => {
    expect(pointInRect(30, 30, r)).toBe(true);
    expect(pointInRect(10, 20, r)).toBe(true);
    expect(pointInRect(50, 40, r)).toBe(true);
    expect(pointInRect(9, 30, r)).toBe(false);
    expect(pointInRect(30, 41, r)).toBe(false);
  });
});

describe("pickLayout", () => {
  const base = { hovered: false, inputOpen: false, paneOpen: false, rosterVisible: false };
  it("is compact while idle, grows on hover, and goes full for anything with text", () => {
    expect(pickLayout({ ...base, state: "default" })).toBe("compact");
    expect(pickLayout({ ...base, state: "dictation_ready" })).toBe("compact");
    expect(pickLayout({ ...base, state: "default", hovered: true })).toBe("hover");
    expect(pickLayout({ ...base, state: "listening", hovered: true })).toBe("voice");
    expect(pickLayout({ ...base, state: "always_listening" })).toBe("voice");
    expect(pickLayout({ ...base, state: "default", inputOpen: true })).toBe("full");
    expect(pickLayout({ ...base, state: "expanding" })).toBe("full");
    expect(pickLayout({ ...base, state: "loading" })).toBe("full");
    expect(pickLayout({ ...base, state: "error" })).toBe("full");
    expect(pickLayout({ ...base, state: "default", paneOpen: true })).toBe("full");
    expect(pickLayout({ ...base, state: "default", rosterVisible: true })).toBe("full");
  });
});

describe("FloatingBar", () => {
  it("starts as a tiny pill: no input, no buttons, no pane", async () => {
    await renderBar();

    expect(bar()).toHaveAttribute("data-state", "default");
    expect(bar()).toHaveAttribute("data-layout", "compact");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
    expect(lastResize()).toEqual({ width: 88, height: 48, anchorY: 24 });
  });

  it("grows on hover to reveal the mic and type buttons, and shrinks back after the animation", async () => {
    vi.useFakeTimers();
    await renderBar();

    await hover(true);
    expect(bar()).toHaveAttribute("data-layout", "hover");
    expect(screen.getByRole("button", { name: "Talk to Juno" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Type to Juno" })).toBeInTheDocument();
    // Growing: the window makes room straight away.
    expect(lastResize()).toEqual({ width: 164, height: 66, anchorY: 33 });

    await hover(false);
    expect(bar()).toHaveAttribute("data-layout", "compact");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    // Shrinking: the window waits for the pill to animate down first.
    expect(lastResize()).toEqual({ width: 164, height: 66, anchorY: 33 });
    act(() => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS);
    });
    expect(lastResize()).toEqual({ width: 88, height: 48, anchorY: 24 });
  });

  it("also treats DOM hover as hover, for when Juno is the active app", async () => {
    await renderBar();

    fireEvent.mouseEnter(bar().parentElement!);
    expect(bar()).toHaveAttribute("data-layout", "hover");
    fireEvent.mouseLeave(bar().parentElement!);
    await settle(LEAVE_VERIFY_MS);
    expect(bar()).toHaveAttribute("data-layout", "compact");
  });

  it("ignores a leave while the cursor is still over the window (resizing under a resting cursor reports one)", async () => {
    await renderBar();

    await hover(true);
    // The window just grew under the cursor; the OS says "left" but the
    // cursor is inside the new, larger window.
    act(() => {
      listenHandlers.get("mouse-left-window")?.({ payload: null });
    });
    await settle(LEAVE_VERIFY_MS);

    expect(bar()).toHaveAttribute("data-layout", "hover");
  });

  it("starts a spoken query to the agent on a mic click, without activating the window", async () => {
    await renderBar();
    await hover(true);

    fireEvent.click(screen.getByRole("button", { name: "Talk to Juno" }));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "start" });
    expect(windowSetFocus).not.toHaveBeenCalled();
  });

  it("shows listening status with a stop control while the mic is open", async () => {
    await renderBar();

    setBarState({ barState: "listening", audioLevel: 0.6 });

    expect(bar()).toHaveAttribute("data-layout", "voice");
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("listening");
    expect(lastResize()).toEqual({ width: 252, height: 66, anchorY: 33 });

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "stop" });
  });

  it("shows a processing state, not the listening look, once the mic closes", async () => {
    await renderBar();

    setBarState({ barState: "listening", audioLevel: 0.6 });
    expect(bar()).toHaveAttribute("data-layout", "voice");

    // The backend sets transcribing the instant the mic stops.
    setBarState({ barState: "transcribing" });
    expect(bar()).toHaveAttribute("data-layout", "full");
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("transcribing");
    // A processing state has a Stop control, and no live audio bars.
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop listening" })).not.toBeInTheDocument();
  });

  it("lights the pill button under the forwarded cursor, even with Juno inactive", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      const label = this.getAttribute?.("aria-label");
      const box =
        label === "Talk to Juno"
          ? { left: 100, top: 20, right: 140, bottom: 48 }
          : label === "Type to Juno"
            ? { left: 150, top: 20, right: 190, bottom: 48 }
            : { left: 0, top: 0, right: 0, bottom: 0 };
      return { ...box, width: box.right - box.left, height: box.bottom - box.top, x: box.left, y: box.top, toJSON() {} } as DOMRect;
    });

    await renderBar();
    await hover(true);
    const mic = screen.getByRole("button", { name: "Talk to Juno" });
    const type = screen.getByRole("button", { name: "Type to Juno" });

    await move(120, 30); // over the mic
    expect(mic).toHaveAttribute("data-phover");
    expect(type).not.toHaveAttribute("data-phover");

    await move(170, 30); // over the type button
    expect(type).toHaveAttribute("data-phover");
    expect(mic).not.toHaveAttribute("data-phover");

    await move(300, 300); // over neither
    expect(mic).not.toHaveAttribute("data-phover");
    expect(type).not.toHaveAttribute("data-phover");
  });

  it("has no stop control for always-listening, which is not a turn", async () => {
    await renderBar();
    setBarState({ barState: "always_listening", isAlwaysListening: true });

    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("always listening");
    expect(screen.queryByRole("button", { name: "Stop listening" })).not.toBeInTheDocument();
  });

  it("opens the input focused on a type click, activating the window and telling the backend", async () => {
    await renderBar();

    const input = await openInput();

    expect(bar()).toHaveAttribute("data-layout", "full");
    expect(input).toHaveFocus();
    expect(windowSetFocus).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("focus", { isFocused: true }),
    );
    expect(lastResize()).toEqual({ width: 467, height: 92, anchorY: 46 });
  });

  it("submits typed input through the standard bar interaction and clears it", async () => {
    await renderBar();
    const input = await openInput();

    fireEvent.change(input, { target: { value: "  open my calendar " } });
    fireEvent.submit(input.closest("form")!);
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("submit", { value: "open my calendar" }),
    );
    expect(input).toHaveValue("");
  });

  it("folds the input away on Escape and on an empty blur, telling the backend", async () => {
    await renderBar();
    await openInput();
    await hover(false);

    fireEvent.keyDown(document, { key: "Escape" });
    await act(async () => {});
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(bar()).toHaveAttribute("data-layout", "compact");
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("blur", { isFocused: false }),
    );

    const input = await openInput();
    act(() => input.blur());
    await settle(BLUR_SETTLE_MS);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("keeps the input through a blur that is immediately followed by a refocus", async () => {
    await renderBar();
    const input = await openInput();

    act(() => input.blur());
    act(() => input.focus());
    await settle(BLUR_SETTLE_MS);

    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("folds an empty input up when the whole window loses focus", async () => {
    await renderBar();
    await openInput();

    act(() => windowFocus.handler?.({ payload: false }));
    await act(async () => {});

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("blur", { isFocused: false }),
    );
  });

  it("keeps the input when it blurs with text in it", async () => {
    await renderBar();
    const input = await openInput();

    fireEvent.change(input, { target: { value: "half a thought" } });
    act(() => input.blur());
    await settle(BLUR_SETTLE_MS);

    expect(screen.getByRole("textbox")).toHaveValue("half a thought");
  });

  it("shows the input while the backend is in its input states even if it was not opened here", async () => {
    await renderBar();

    setBarState({ barState: "expanding" });
    const input = screen.getByPlaceholderText("Ask Juno");
    expect(input).toBeEnabled();
    fireEvent.change(input, { target: { value: "hel" } });
    expect(input).toHaveValue("hel");
  });

  it("drags the window from anywhere once the mouse moves, and swallows the click that follows", async () => {
    await renderBar();
    await hover(true);
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 12, clientY: 11 }); // under the threshold
    expect(startDragging).not.toHaveBeenCalled();
    fireEvent.mouseMove(mic, { clientX: 30, clientY: 20 });
    expect(startDragging).toHaveBeenCalledTimes(1);

    fireEvent.mouseUp(mic);
    fireEvent.click(mic);
    await act(async () => {});
    expect(invoke).not.toHaveBeenCalledWith("agent_voice", expect.anything());
  });

  it("treats a press and release without movement as the click it is", async () => {
    await renderBar();
    await hover(true);
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 11, clientY: 10 });
    fireEvent.mouseUp(mic);
    fireEvent.click(mic);
    await act(async () => {});

    expect(startDragging).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "start" });
  });

  it("never drags from the text input, so text can be selected", async () => {
    await renderBar();
    const input = await openInput();

    fireEvent.mouseDown(input, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(input, { clientX: 60, clientY: 10 });

    expect(startDragging).not.toHaveBeenCalled();
  });

  it("puts the caret back in an open input when the window becomes key", async () => {
    await renderBar();
    const input = await openInput();
    fireEvent.change(input, { target: { value: "half a thought" } });
    invoke.mockClear();
    act(() => input.blur());
    expect(input).not.toHaveFocus();

    act(() => windowFocus.handler?.({ payload: true }));
    await act(async () => {});

    expect(input).toHaveFocus();
    // Keystrokes only reach the page once the webview is first responder.
    expect(webviewSetFocus).toHaveBeenCalled();
  });

  it("does not expand into the input just because the window became key", async () => {
    await renderBar();

    act(() => windowFocus.handler?.({ payload: true }));
    await act(async () => {});

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", interaction("focus"));
  });

  it("opens the chat pane with the user's message when the backend announces a query", async () => {
    await renderBar();

    submitUserMessage("Play my liked songs on Spotify");

    const pane = screen.getByTestId("bar-chat-pane");
    expect(pane).toBeInTheDocument();
    expect(pane).toHaveClass("dark");
    expect(screen.getByText("Play my liked songs on Spotify")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("working");
    expect(bar()).toHaveAttribute("data-layout", "full");
    expect(lastResize()).toEqual({ width: 467, height: 460, anchorY: 46 });
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
    vi.useFakeTimers();
    await renderBar();

    submitUserMessage("Hello");
    // Still processing: Escape belongs to the Rust stop-key monitor.
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();

    streamAssistant("m1", "Hi.");
    fire("agent-active", false);
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS);
    });
    expect(lastResize()).toEqual({ width: 88, height: 48, anchorY: 24 });
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

  it("hands an open input over to a voice turn that starts from the hotkey", async () => {
    await renderBar();
    await openInput();

    setBarState({ barState: "listening" });

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(bar()).toHaveAttribute("data-layout", "voice");
  });
});
