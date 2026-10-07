import { StrictMode } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  BLUR_SETTLE_MS,
  FloatingBar,
  LEAVE_VERIFY_MS,
  PILL_STEADY_SPEC,
  pillFootprint,
  pickLayout,
  pointInRect,
} from "../FloatingBar";
import { getSteady } from "@/lib/steadyFrame";

// ── Tauri + hook mocks ───────────────────────────────────────────────

// The window's reported outer top-left. `set_bar_frame` moves it, as the real
// command moves the window; a test can also move it to a drop point.
const outerPos = vi.hoisted(() => ({ x: 100, y: 100 }));
// Resolves to nothing unless a test teaches it a command's answer, and moves
// the mocked window when the bar sets its frame.
const defaultInvoke = vi.hoisted(() => (...args: unknown[]): Promise<unknown> => {
  if (args[0] === "set_bar_frame") {
    const a = args[1] as { x: number; y: number };
    outerPos.x = a.x;
    outerPos.y = a.y;
  }
  return Promise.resolve();
});
const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged } = vi.hoisted(() => ({
  invoke: vi.fn(defaultInvoke),
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
  // The bar broadcasts snap-wells-show/hide to drive the drop indicator overlay.
  emit: vi.fn(async () => {}),
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
const windowSetPosition = vi.hoisted(() => vi.fn(() => Promise.resolve()));
// One 1000×800 monitor at the origin by default; a test can swap this out.
const monitors = vi.hoisted(() => ({
  value: [{ position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 }],
}));
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
    setPosition: windowSetPosition,
    outerPosition: async () => ({ ...outerPos }),
    outerSize: async () => ({ width: 164, height: 66 }),
    scaleFactor: async () => 1,
  }),
  availableMonitors: async () => monitors.value,
  PhysicalPosition: class {
    x: number;
    y: number;
    constructor(x: number, y: number) {
      this.x = x;
      this.y = y;
    }
  },
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

/**
 * The pill is drawn from the frame the window has been given, so a state
 * change shows only once the (mocked) resize has resolved: every driver flushes
 * that promise before handing back.
 */
const flush = () => act(async () => {});

const setBarState = (overrides: Record<string, unknown>) =>
  act(async () => {
    listenHandlers.get("bar-state-update")?.({ payload: barState(overrides) });
  });

const fire = (event: string, payload: unknown) =>
  act(async () => {
    eventHandlers.get(event)?.(payload);
  });

/**
 * The native tracking area reporting the mouse crossing the window's edge.
 * A leave is only believed once the cursor is confirmed outside the window,
 * so leaving also parks the cursor outside and lets that check run.
 */
const hover = async (inside: boolean) => {
  const p = pillCentre();
  cursor.x = inside ? p.x : -5000;
  cursor.y = inside ? p.y : -5000;
  act(() => {
    listenHandlers.get(inside ? "mouse-entered-window" : "mouse-left-window")?.({
      payload: null,
    });
  });
  if (!inside) await settle(LEAVE_VERIFY_MS);
  await flush();
};

/** The resting footprint's centre on screen: inside what the pill draws in every state. */
function pillCentre() {
  const l = getSteady("floating-bar")?.layout;
  if (!l) return { x: 150, y: 130 };
  return {
    x: l.origin.x + (l.anchor.x + l.anchor.width / 2) * l.scaleFactor,
    y: l.origin.y + (l.anchor.y + l.anchor.height / 2) * l.scaleFactor,
  };
}

/** Every frame the bar has set on its window, in order. */
const frames = () =>
  (invoke.mock.calls as unknown[][])
    .filter((c) => c[0] === "set_bar_frame")
    .map((c) => c[1] as { x: number; y: number; width: number; height: number });

/** The hit regions the bar last reported. */
const lastRegions = () => {
  const calls = (invoke.mock.calls as unknown[][]).filter((c) => c[0] === "set_bar_hit_regions");
  return (calls[calls.length - 1]?.[1] as { regions: unknown } | undefined)?.regions;
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

const streamAssistant = async (id: string, text: string) => {
  await fire("agent-stream-start", { message_id: id });
  await fire("agent-text-stream", { chunk: text, message_id: id });
  await fire("agent-stream-end", { message_id: id, complete_text: text, agent_state: "Finished" });
};

async function renderBar() {
  const utils = render(<FloatingBar />);
  // Let the async listen() registrations settle.
  await act(async () => {});
  return utils;
}

const bar = () => screen.getByTestId("floating-bar");
/**
 * The footprint the pill last reported drawing (pill plus pad, roster, pane):
 * what the window used to be resized to, and what now takes the mouse inside
 * a window that never resizes. Asserted with `toMatchObject` so a test says
 * only what it is about.
 */
const lastResize = () =>
  (lastRegions() as Array<Record<string, number>> | undefined)?.[0];

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
  windowSetPosition.mockClear();
  outerPos.x = 100;
  outerPos.y = 100;
  monitors.value = [
    { position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 },
  ];
  listenHandlers.clear();
  eventHandlers.clear();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  // A test may teach invoke to answer a command; the next one starts mute.
  invoke.mockImplementation(defaultInvoke);
});

// ── Tests ────────────────────────────────────────────────────────────

describe("pillFootprint", () => {
  it("is exactly the pill plus one pad each side, anchored on the band's near edge in every layout", () => {
    expect(pillFootprint({ layout: "compact", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 88, height: 76 });
    expect(pillFootprint({ layout: "hover", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 180, height: 76 });
    expect(pillFootprint({ layout: "voice", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 292, height: 76 });
    // Status shares voice's width so the bar does not lurch wider the moment
    // the mic closes, for a word and a stop button.
    expect(pillFootprint({ layout: "status", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 292, height: 76 });
    // Full used to carry its own pad and band (24 and 44 against 16 and 34),
    // which moved the anchor by 13px every time the pill went full: the lurch
    // on pane close. Same height, same anchor, whatever the layout.
    expect(pillFootprint({ layout: "full", paneOpen: false, rosterVisible: false }))
      .toEqual({ width: 451, height: 76 });
    expect(
      pillFootprint({ layout: "full", paneOpen: false, rosterVisible: false, composerGrowth: 36 }),
    ).toEqual({ width: 451, height: 112 });
    expect(pillFootprint({ layout: "full", paneOpen: true, rosterVisible: false }))
      .toEqual({ width: 451, height: 444 });
    expect(pillFootprint({ layout: "full", paneOpen: true, rosterVisible: true }))
      .toEqual({ width: 451, height: 478 });
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
    // Working states carry a label and one control, not an input, so they get
    // the status width rather than the full 419px.
    expect(pickLayout({ ...base, state: "loading" })).toBe("status");
    expect(pickLayout({ ...base, state: "error" })).toBe("status");
    expect(pickLayout({ ...base, state: "transcribing" })).toBe("status");
    expect(pickLayout({ ...base, state: "default", paneOpen: true })).toBe("full");
    expect(pickLayout({ ...base, state: "default", rosterVisible: true })).toBe("full");
  });

  it("goes full while Juno holds the pointer, so the bar can say so in words", () => {
    expect(pickLayout({ ...base, state: "default", driving: true })).toBe("full");
    expect(pickLayout({ ...base, state: "default", hovered: true, driving: true })).toBe(
      "full",
    );
  });
});

describe("FloatingBar while Juno is driving", () => {
  const driving = (active: boolean, targetApp?: string) =>
    fire("input-control-state", { active, tool: "computer", target_app: targetApp });

  it("says which app Juno has the pointer in", async () => {
    await renderBar();
    await driving(true, "Safari");

    expect(bar()).toHaveAttribute("data-driving", "");
    expect(bar()).toHaveAttribute("data-layout", "full");
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent(
      "Juno is using the mouse in Safari",
    );
    expect(screen.getByTestId("floating-bar-driving-dot")).toBeInTheDocument();
  });

  it("still says it plainly when no app was named", async () => {
    await renderBar();
    await driving(true);

    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent(
      "Juno is using the mouse",
    );
  });

  it("clears the moment Juno gives the pointer back", async () => {
    await renderBar();
    await driving(true, "Safari");
    await driving(false);

    expect(bar()).not.toHaveAttribute("data-driving");
    expect(bar()).toHaveAttribute("data-layout", "compact");
    expect(screen.queryByTestId("floating-bar-driving-dot")).not.toBeInTheDocument();
  });

  it("keeps the text input out of the way while the pointer is taken", async () => {
    await renderBar();
    await openInput();
    await driving(true, "Safari");

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent(
      "Juno is using the mouse in Safari",
    );
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
    // The launch placement sets the steady frame, in the well, and nothing
    // else touches the window; the pill draws compact inside it.
    expect(invoke).toHaveBeenCalledWith(
      "set_bar_frame",
      expect.objectContaining({ width: 452, height: 574 }),
    );
    expect(lastResize()).toMatchObject({ width: 88, height: 76 });
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });

  it("grows on hover to reveal the mic and type buttons, and shrinks back after the animation", async () => {
    vi.useFakeTimers();
    await renderBar();

    await hover(true);
    expect(bar()).toHaveAttribute("data-layout", "hover");
    expect(screen.getByRole("button", { name: "Talk to Juno" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Type to Juno" })).toBeInTheDocument();
    // The footprint grows at once; the window has the room already.
    expect(lastResize()).toMatchObject({ width: 180, height: 76 });

    await hover(false);
    expect(bar()).toHaveAttribute("data-layout", "compact");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(lastResize()).toMatchObject({ width: 88, height: 76 });
    expect(frames()).toHaveLength(1);
  });

  it("also treats DOM hover as hover, for when Juno is the active app", async () => {
    await renderBar();

    fireEvent.mouseEnter(bar().parentElement!);
    await flush();
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

    await setBarState({ barState: "listening", audioLevel: 0.6 });

    expect(bar()).toHaveAttribute("data-layout", "voice");
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("listening");
    // The voice pill, plus room for the expand control while the chat is closed.
    expect(lastResize()).toMatchObject({ width: 324, height: 76 });

    // "Stop" used to be the only control and it submitted what you had said.
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "stop" });
  });

  it("can abandon a sentence without sending it", async () => {
    await renderBar();
    await setBarState({ barState: "listening", audioLevel: 0.6 });

    fireEvent.click(screen.getByRole("button", { name: "Cancel without sending" }));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "cancel" });
    expect(invoke).not.toHaveBeenCalledWith("agent_voice", { action: "stop" });
  });

  it("can switch from talking to typing without sending", async () => {
    await renderBar();
    await setBarState({ barState: "listening", audioLevel: 0.6 });

    fireEvent.click(screen.getByRole("button", { name: "Type instead" }));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "cancel" });
  });

  it("renders a live partial as dimmed provisional text, then solid on final", async () => {
    await renderBar();

    await setBarState({ barState: "listening", isDictationMode: true, audioLevel: 0.4 });

    // With live transcription on, the backend sets transcribing with the
    // cumulative provisional text every ~600ms while the mic is still open.
    await setBarState({
      barState: "transcribing",
      isDictationMode: true,
      transcriptionText: "hello there",
      transcriptionProvisional: true,
    });
    const status = screen.getByTestId("floating-bar-status");
    expect(status).toHaveTextContent("hello there");
    expect(status).toHaveClass("italic");

    // A later partial replaces, never appends, and stays provisional.
    await setBarState({
      barState: "transcribing",
      isDictationMode: true,
      transcriptionText: "hello there world",
      transcriptionProvisional: true,
    });
    expect(status).toHaveTextContent("hello there world");
    expect(status).not.toHaveTextContent("hello there hello there");

    // The final result swaps the same span to solid.
    await setBarState({
      barState: "transcribing",
      transcriptionText: "hello there world",
      transcriptionProvisional: false,
    });
    expect(status).toHaveTextContent("hello there world");
    expect(status).not.toHaveClass("italic");
  });

  it("shows a processing state, not the listening look, once the mic closes", async () => {
    await renderBar();

    await setBarState({ barState: "listening", audioLevel: 0.6 });
    expect(bar()).toHaveAttribute("data-layout", "voice");

    // The backend sets transcribing the instant the mic stops. The bar keeps
    // the same width it had while listening: the old behaviour jumped from
    // 220px to 419px mid-sentence to show one more word.
    await setBarState({ barState: "transcribing" });
    expect(bar()).toHaveAttribute("data-layout", "status");
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("transcribing");
    // A processing state has a Stop control, and no live audio bars.
    expect(screen.getByRole("button", { name: "Stop Juno" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
  });

  it.each([
    ["listening", { barState: "listening", audioLevel: 0.6 }],
    ["dictating", { barState: "dictating", isDictationMode: true }],
    [
      "live words while you talk",
      { barState: "transcribing", transcriptionText: "testing", transcriptionProvisional: true },
    ],
  ])("keeps Send, Type instead and Cancel in every recording state: %s", async (_name, state) => {
    await renderBar();
    await setBarState(state);

    // The owner's report: once speech was detected the bar switched to the
    // transcript with only a stop square, and there was no way to send with
    // the mouse.
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Type instead" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel without sending" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop Juno" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "stop" });
  });

  it("cancels the turn, not the coordinated stop, from the X while live words arrive", async () => {
    await renderBar();
    await setBarState({
      barState: "transcribing",
      transcriptionText: "testing",
      transcriptionProvisional: true,
    });

    fireEvent.click(screen.getByRole("button", { name: "Cancel without sending" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "cancel" });
    expect(invoke).not.toHaveBeenCalledWith("stop_all_operations");
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

  it("offers a cancel, and nothing to send, while a wake phrase is being taken down", async () => {
    await renderBar();
    await setBarState({ barState: "always_listening", isAlwaysListening: true });

    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("always listening");
    // The engine decides when the sentence ends, so there is nothing to commit
    // by hand. There is something to stop, though, and every other state that
    // has something to stop offers the same X.
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel without sending" }));
    // The capture belongs to the always-listening engine, which only the
    // coordinated stop can reach; it re-arms the wake phrase on its way out.
    expect(invoke).toHaveBeenCalledWith("stop_all_operations");
    expect(invoke).not.toHaveBeenCalledWith("agent_voice", { action: "cancel" });
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
    expect(lastResize()).toMatchObject({ width: 451, height: 76 });
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

    await setBarState({ barState: "expanding" });
    const input = screen.getByPlaceholderText("Ask Juno");
    expect(input).toBeEnabled();
    fireEvent.change(input, { target: { value: "hel" } });
    expect(input).toHaveValue("hel");
  });

  // The drag gesture and its snap into a well are no longer the Pill's own:
  // they live in `useBarDrag` (hooks/useDragWindow.ts), shared by every bar
  // appearance. Their tests moved with them, unchanged, to
  // hooks/__tests__/useBarDrag.test.tsx.

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

    await submitUserMessage("Play my liked songs on Spotify");

    const pane = screen.getByTestId("bar-chat-pane");
    expect(pane).toBeInTheDocument();
    expect(pane).toHaveClass("dark");
    expect(screen.getByText("Play my liked songs on Spotify")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("working");
    expect(bar()).toHaveAttribute("data-layout", "full");
    expect(lastResize()).toMatchObject({ width: 451, height: 444 });
  });

  it("streams the assistant response into the pane and settles when the agent goes idle", async () => {
    await renderBar();

    await submitUserMessage("What time is it?");
    await streamAssistant("m1", "It is half past nine.");
    // `agent-active` is the run, and only the run: this is the agent reporting
    // that it has finished working.
    await fire("agent-active", false);

    expect(screen.getByText("It is half past nine.")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("esc to close");
  });

  it("keeps showing progress when the microphone closes mid-run", async () => {
    await renderBar();

    await submitUserMessage("Open Safari");
    // A spoken query closes the microphone as soon as the person stops
    // speaking, which is before the agent has done anything. While capture and
    // execution shared `agent-active`, this teardown reached the pane as "the
    // agent stopped" and the status settled while the agent worked on.
    await fire("agent-capture-active", false);

    expect(screen.getByTestId("bar-chat-pane-status")).toHaveTextContent("working");
  });

  it("keeps the pill as an input for follow-ups while the pane is open", async () => {
    await renderBar();

    await submitUserMessage("Hello");
    await streamAssistant("m1", "Hi there.");
    await fire("agent-active", false);
    await setBarState({ barState: "default" });

    expect(screen.getByPlaceholderText("Follow up…")).toBeInTheDocument();
  });

  it("shows status and a Stop control instead of the input while the agent works", async () => {
    await renderBar();

    await submitUserMessage("Open Safari");
    await setBarState({ barState: "loading", agentState: "working", isAgentWorking: true });

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByTestId("floating-bar-status")).toHaveTextContent("working");

    fireEvent.click(screen.getByRole("button", { name: "Stop Juno" }));
    expect(invoke).toHaveBeenCalledWith("stop_all_operations");
  });

  it("returns the bar to rest on one Escape, whatever is running", async () => {
    vi.useFakeTimers();
    await renderBar();

    await submitUserMessage("Hello");
    // Mid-run. One press reports the key to Rust (which decides what stopping
    // means) and puts the pane away; it does not take two presses, and it is
    // not left to whether the bar happens to be the focused window.
    fireEvent.keyDown(document, { key: "Escape" });
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction("escape"));
    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();

    await streamAssistant("m1", "Hi.");
    await fire("agent-active", false);
    await setBarState({ barState: "default" });
    expect(lastResize()).toMatchObject({ width: 88, height: 76 });
  });

  it("dismisses the pane when Rust reports an Escape it had nothing to stop", async () => {
    await renderBar();

    await submitUserMessage("Hello");
    await streamAssistant("m1", "Hi.");
    await fire("agent-active", false);
    await setBarState({ barState: "default" });
    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();

    // The person pressed Escape in another app: Rust's passive monitor saw it,
    // found nothing running, and asked the bar to put itself away.
    await fire("bar-dismiss-pane", null);

    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
  });

  it("reopens a dismissed pane when the next query arrives", async () => {
    await renderBar();

    await submitUserMessage("First");
    await streamAssistant("m1", "One.");
    await fire("agent-active", false);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss conversation" }));
    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();

    await submitUserMessage("Second");

    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();
    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.getByText("Second")).toBeInTheDocument();
  });

  it("clears the conversation but stays open and ready to type on New chat", async () => {
    await renderBar();

    await submitUserMessage("Hello");
    await streamAssistant("m1", "Hi.");
    await fire("agent-active", false);
    fireEvent.click(screen.getByRole("button", { name: "New chat" }));
    await act(async () => {});

    // The pane used to vanish here, leaving no control anywhere to bring it
    // back: asking for a new chat closed the chat.
    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();
    expect(screen.queryByText("Hello")).not.toBeInTheDocument();
    // And the backend gets a fresh conversation, so the agent stops appending
    // to the old one behind an apparently empty pane.
    expect(invoke).toHaveBeenCalledWith("new_conversation");

    await submitUserMessage("Again");
    expect(screen.getByText("Again")).toBeInTheDocument();
  });

  /** A pane open on an empty conversation: the only route to the example prompts. */
  async function openEmptyPane() {
    await renderBar();
    await submitUserMessage("Hello");
    await streamAssistant("m1", "Hi.");
    await fire("agent-active", false);
    await setBarState({ barState: "default" });
    fireEvent.click(screen.getByRole("button", { name: "New chat" }));
    await act(async () => {});
  }

  /** One starter as Rust's get_starters returns it. */
  const STARTER = { id: "screen", title: "What's on my screen?", prompt: "What's on my screen? Look at it and tell me what I'm working on." };

  it("sends an example prompt from the pane's empty state the way a typed follow-up goes", async () => {
    // The bar's own connectivity probe; the mock returns nothing by default,
    // which reads as an error.
    invoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === "check_server_status"
          ? { backend_running: true }
          : command === "get_starters"
            ? [STARTER]
            : undefined,
      ),
    );
    await openEmptyPane();

    const screenshot = await screen.findByRole("button", { name: STARTER.title });
    expect(screenshot).toBeEnabled();
    expect(screen.queryByTestId("example-prompts-connecting")).not.toBeInTheDocument();

    fireEvent.click(screenshot);
    await act(async () => {});

    // The same ui_handle_interaction submit the pill input dispatches; this
    // used to be a deliberate no-op, so the buttons did nothing in the bar.
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("submit", { value: STARTER.prompt }),
    );
    expect(invoke).not.toHaveBeenCalledWith("dispatch_query", expect.anything());
  });

  it("sends a development test command from the pane the same way as a production prompt", async () => {
    invoke.mockImplementation((command: unknown) => {
      if (command === "check_server_status") return Promise.resolve({ backend_running: true });
      // A debug build: the dev commands drawer is offered under the prompts.
      if (command === "get_debug_mode") return Promise.resolve(true);
      return Promise.resolve();
    });
    await openEmptyPane();

    fireEvent.click(await screen.findByText("Development test commands"));
    fireEvent.click(screen.getByRole("button", { name: "Mouse Square" }));
    await act(async () => {});

    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("submit", {
        value: "Move your mouse in a perfect square pattern on the screen, then return to center",
      }),
    );
  });

  it("holds the example prompts behind a loader until the backend answers, then lets them send", async () => {
    let connected: (status: { backend_running: boolean }) => void = () => {};
    invoke.mockImplementation((command: unknown) =>
      command === "check_server_status"
        ? new Promise<{ backend_running: boolean }>((resolve) => {
            connected = resolve;
          })
        : Promise.resolve(command === "get_starters" ? [STARTER] : undefined),
    );
    await openEmptyPane();

    await screen.findByRole("button", { name: STARTER.title });
    const screenshot = () => screen.getByRole("button", { name: STARTER.title });
    expect(screenshot()).toBeDisabled();
    expect(screen.getByTestId("example-prompts-connecting")).toHaveTextContent("Connecting…");
    fireEvent.click(screenshot());
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", interaction("submit"));

    await act(async () => {
      connected({ backend_running: true });
    });

    expect(screenshot()).toBeEnabled();
    expect(screen.queryByTestId("example-prompts-connecting")).not.toBeInTheDocument();
    fireEvent.click(screenshot());
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction("submit", { value: STARTER.prompt }),
    );
  });

  it("always offers a way into the chat from the idle bar", async () => {
    await renderBar();
    // With no conversation at all, hovering the pill still offers the chat.
    fireEvent.mouseEnter(bar());
    await flush();
    expect(screen.getByRole("button", { name: "Open chat" })).toBeInTheDocument();
  });

  it("hands an open input over to a voice turn that starts from the hotkey", async () => {
    await renderBar();
    await openInput();

    await setBarState({ barState: "listening" });

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(bar()).toHaveAttribute("data-layout", "voice");
  });
});

// ── The steady frame ─────────────────────────────────────────────────
//
// Every jump the pill ever showed came from a window resize: AppKit moves the
// window and WebKit re-lays the page on different display frames, so anything
// pinned to a right or bottom edge lurched for a frame. The Pill's window is
// now sized once for its largest state and placed once per well. These pin
// that: one frame per well, never one per state, and the docked corner of
// what is drawn sitting on the well whatever the state.

describe("FloatingBar steady frame", () => {
  const column = () => screen.getByTestId("floating-bar-column");

  /** Drag the pill and drop it at a physical point; the release settles it into the nearest well. */
  async function dropAt(x: number, y: number) {
    await hover(true);
    const mic = screen.getByRole("button", { name: "Talk to Juno" });
    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 30, clientY: 20 });
    outerPos.x = x;
    outerPos.y = y;
    // Released with the cursor where it grabbed the window; the settle reads
    // the landing from the cursor.
    cursor.x = x + 10;
    cursor.y = y + 10;
    fireEvent.mouseUp(window);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 500));
    });
    await hover(false);
  }

  it("places the window once, at the largest footprint, with the resting pill on the top-right well", async () => {
    await renderBar();
    await waitFor(() => expect(frames()).toHaveLength(1));
    // 1000x800 display: the resting 88x76 footprint's top-right well is at
    // (896, 36); the 452-wide window extends left from it.
    expect(frames()[0]).toEqual({ x: 532, y: 36, width: 452, height: 574 });
    expect(PILL_STEADY_SPEC.max).toEqual({ width: 452, height: 574 });
    // Pinned to the window's right and top edges, which are the well's.
    expect(column()).toHaveStyle({ right: "0px", top: "0px" });
    expect(column()).toHaveClass("items-end");
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });

  it("never touches the window through a whole turn, and the docked corner never moves", async () => {
    await renderBar();
    await waitFor(() => expect(frames()).toHaveLength(1));
    const corners = new Set<string>();
    const record = () => {
      const r = lastRegions() as Array<{ x: number; y: number; width: number; height: number }>;
      corners.add(`${r[0].x + r[0].width},${r[0].y}`);
    };

    await hover(true);
    record();
    await setBarState({ barState: "listening", audioLevel: 0.6 });
    record();
    await setBarState({ barState: "transcribing" });
    record();
    await submitUserMessage("Open Safari");
    await setBarState({ barState: "loading", agentState: "working", isAgentWorking: true });
    record();
    await streamAssistant("m1", "Done.");
    await fire("agent-active", false);
    await setBarState({ barState: "finishing" });
    await setBarState({ barState: "default" });
    record();
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.keyDown(document, { key: "Escape" });
    await hover(false);
    record();

    // One frame, set at launch. Nothing a state did reached the window.
    expect(frames()).toHaveLength(1);
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    // The drawn footprint's top-right corner is the window's top-right corner
    // in every state: the right edge and the top edge are the well's.
    expect([...corners]).toEqual(["452,0"]);
    expect(column()).toHaveStyle({ right: "0px", top: "0px" });
  });

  it("tells the backend what it draws, so the transparent rest of the window lets clicks through", async () => {
    const { unmount } = await renderBar();
    await waitFor(() => expect(lastRegions()).toBeDefined());
    expect(lastRegions()).toEqual([{ x: 364, y: 0, width: 88, height: 76 }]);

    await setBarState({ barState: "listening", audioLevel: 0.6 });
    // Listening carries the expand control while the chat is closed.
    expect(lastRegions()).toEqual([{ x: 128, y: 0, width: 324, height: 76 }]);

    // Another look taking the window gets every mouse event back.
    unmount();
    expect(invoke).toHaveBeenCalledWith("set_bar_hit_regions", { regions: null });
  });

  it("opens the pane upward at a bottom well, and swaps the frame once, on the drop", async () => {
    await renderBar();
    await waitFor(() => expect(frames()).toHaveLength(1));
    await dropAt(850, 700);
    await waitFor(() => expect(frames()).toHaveLength(2));
    // Bottom-right well for the resting footprint: (896, 708). The window
    // now extends up and left from it, so its bottom-right is the well's.
    expect(frames()[1]).toEqual({ x: 532, y: 210, width: 452, height: 574 });
    expect(column()).toHaveStyle({ right: "0px", bottom: "0px" });
    // Visible again once the swap has reached the screen.
    await waitFor(() => expect(column()).toHaveClass("opacity-100"));

    await submitUserMessage("Hello");
    // The pane is drawn above the pill, and the window did not move for it.
    const order = Array.from(column().querySelectorAll("[data-testid]")).map((el) =>
      el.getAttribute("data-testid"),
    );
    expect(order.indexOf("bar-chat-pane")).toBeLessThan(order.indexOf("floating-bar"));
    expect(frames()).toHaveLength(2);
    expect(lastResize()).toMatchObject({ width: 451, height: 444 });
    // Bottom edge of what is drawn: the window's bottom, which is the well's.
    const r = lastRegions() as Array<{ y: number; height: number }>;
    expect(r[0].y + r[0].height).toBe(574);
  });

  it("grows the drawn footprint at the far edge only when the typed text wraps", async () => {
    // Three lines of text: the textarea measures 54px against an 18px line.
    vi.spyOn(HTMLTextAreaElement.prototype, "scrollHeight", "get").mockReturnValue(54);
    await renderBar();
    const input = await openInput();

    fireEvent.change(input, { target: { value: "one\ntwo\nthree" } });
    await flush();

    expect(lastResize()).toMatchObject({ y: 0, width: 451, height: 112 });
    expect(frames()).toHaveLength(1);
    // The band grows with the text so the pane, when there is one, slides
    // with the pill's bottom edge rather than jumping.
    expect(bar().parentElement).toHaveStyle({ height: "80px" });
  });

  it("does not animate the pill under Reduce Motion", async () => {
    await renderBar();
    expect(bar()).toHaveClass("motion-reduce:transition-none");
    expect(bar().parentElement).toHaveClass("motion-reduce:transition-none");
    await waitFor(() => expect(column()).toHaveClass("motion-reduce:transition-none"));
  });

  it("places and shows the bar once under StrictMode, where every effect runs twice", async () => {
    render(
      <StrictMode>
        <FloatingBar />
      </StrictMode>,
    );
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("show_bar_when_ready"));
    await act(async () => {});

    const calls = (invoke.mock.calls as unknown[][]).map((c) => c[0]);
    expect(calls.filter((c) => c === "set_bar_frame")).toHaveLength(1);
    expect(calls.filter((c) => c === "show_bar_when_ready")).toHaveLength(1);
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });
});

describe("FloatingBar expand control", () => {
  it("offers Open chat while Juno works with the chat closed, ahead of Stop, and it opens the chat", async () => {
    await renderBar();
    await setBarState({ barState: "loading", agentState: "working", isAgentWorking: true });

    expect(screen.queryByTestId("bar-chat-pane")).not.toBeInTheDocument();
    const names = screen.getAllByRole("button").map((b) => b.getAttribute("aria-label"));
    expect(names).toContain("Open chat");
    // Stop keeps its place at the trailing edge.
    expect(names[names.length - 1]).toBe("Stop Juno");
    expect(names.indexOf("Open chat")).toBeLessThan(names.indexOf("Stop Juno"));

    fireEvent.click(screen.getByRole("button", { name: "Open chat" }));
    await flush();
    expect(screen.getByTestId("bar-chat-pane")).toBeInTheDocument();
    // With the chat on screen there is nothing to open.
    expect(screen.queryByRole("button", { name: "Open chat" })).not.toBeInTheDocument();
  });

  it("is offered while listening and on an error, and hidden while the chat window is up", async () => {
    await renderBar();
    await setBarState({ barState: "listening", audioLevel: 0.4 });
    expect(screen.getByRole("button", { name: "Open chat" })).toBeInTheDocument();
    await setBarState({ barState: "error", currentError: "Something broke" });
    expect(screen.getByRole("button", { name: "Open chat" })).toBeInTheDocument();

    await fire("bar-main-window-opened", null);
    expect(screen.queryByRole("button", { name: "Open chat" })).not.toBeInTheDocument();
  });
});
