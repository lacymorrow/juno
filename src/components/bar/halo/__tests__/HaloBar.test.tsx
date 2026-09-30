import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { CLOSE_MS, LINGER_MS, NARROW_WINDOW, SHRINK_DELAY_MS, tickAngle, windowFor } from "../haloModel";

// ── Tauri and hook stand-ins ────────────────────────────────────────

const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged, chat } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(true)),
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  resizeWindowIfChanged: vi.fn((_size: { width: number; height: number }) => Promise.resolve()),
  chat: {
    messages: [] as ChatMessage[],
    isProcessing: false,
    handleApprovalUpdate: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    listenHandlers.set(event, handler);
    return () => listenHandlers.delete(event);
  }),
  emit: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    onFocusChanged: vi.fn(async () => () => {}),
    startDragging: vi.fn(async () => {}),
  }),
}));
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    eventHandlers.set(event, handler);
  },
}));
vi.mock("@/hooks/useWindowSize", () => ({
  useWindowSize: () => ({ resizeWindowIfChanged }),
}));
vi.mock("@/hooks/useBarConversation", () => ({
  useBarConversation: () => chat,
}));
vi.mock("@/hooks/useSkillAutocomplete", () => ({
  useSkillAutocomplete: () => ({
    open: false,
    suggestions: [],
    selectedIndex: 0,
    ghostText: "",
    accept: vi.fn(),
    setSelectedIndex: vi.fn(),
    handleKeyDown: vi.fn(),
  }),
}));
vi.mock("@/components/input-control/InputControlNotices", () => ({
  InputControlNotices: () => null,
}));
vi.mock("@/components/ui/mixed-content-renderer", () => ({
  MixedContentRenderer: ({ content }: { content: string }) => <div data-testid="answer">{content}</div>,
}));
vi.mock("motion/react", async () => {
  const React = await import("react");
  const strip = (props: Record<string, unknown>) => {
    const { initial, animate, exit, transition, ...rest } = props;
    void initial;
    void animate;
    void exit;
    void transition;
    return rest;
  };
  const plain = (tag: string) =>
    React.forwardRef<HTMLElement, Record<string, unknown>>((props, ref) =>
      React.createElement(tag, { ref, ...strip(props) }),
    );
  return {
    useReducedMotion: () => true,
    useMotionValue: (v: number) => ({ get: () => v, set: () => {} }),
    animate: () => ({ stop: () => {} }),
    motion: { div: plain("div"), g: plain("g") },
  };
});

import { HaloBar } from "../HaloBar";

const frame = (barState: string, extra: Record<string, unknown> = {}) => ({
  barState,
  inputValue: "",
  lastSubmittedValue: "",
  currentError: null,
  transcriptionText: "",
  spokenText: "",
  voiceMode: UI.VOICE_MODES_IDLE,
  audioLevel: 0,
  isAgentWorking: false,
  isDictationMode: false,
  isAlwaysListening: false,
  agentState: null,
  ...extra,
});

async function send(state: string, extra: Record<string, unknown> = {}) {
  await act(async () => {
    listenHandlers.get(EVENTS.BAR_STATE_UPDATE)?.({ payload: frame(state, extra) });
  });
}

const ring = () => screen.getByTestId("halo-ring");
const verb = () => ring().getAttribute("data-verb");
const sheet = () => screen.getByTestId("halo").getAttribute("data-sheet");

const user = (content: string): ChatMessage => ({ role: "user", content, timestamp: 1 });
const tool = (content: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
  role: "tool_call_request",
  content,
  tool_name: "t",
  ...extra,
});

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  chat.messages = [];
  chat.isProcessing = false;
});

afterEach(() => {
  vi.useRealTimers();
});

describe("HaloBar", () => {
  it("rests as a thin ring, then fills with your level and shows your words as you speak", async () => {
    render(<HaloBar />);
    expect(verb()).toBe("rest");
    expect(sheet()).toBe("closed");
    await send(UI.BAR_STATES_LISTENING, { audioLevel: 0.6, transcriptionText: "Send the draft to Maya" });
    expect(verb()).toBe("fill");
    expect(ring().getAttribute("data-level")).toBe("0.60");
    expect(screen.getByTestId("halo-gauge").getAttribute("stroke")).toBe("#0A84FF");
    expect(screen.getByTestId("halo-caption")).toHaveTextContent("Send the draft to Maya");
    expect(sheet()).toBe("open");
  });

  it("fills green while dictating", async () => {
    render(<HaloBar />);
    await send(UI.BAR_STATES_DICTATING, { audioLevel: 0.3, transcriptionText: "Hello" });
    expect(screen.getByTestId("halo-gauge").getAttribute("stroke")).toBe("#30D158");
  });

  it("grows the window before the sheet opens, and shrinks it after the sheet folds", async () => {
    render(<HaloBar />);
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith(NARROW_WINDOW);
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING);
    // Growing: the resize came first, at once, to the wide window.
    expect(resizeWindowIfChanged).toHaveBeenCalledWith(windowFor(true, 0));
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_DEFAULT);
    expect(sheet()).toBe("closed");
    // Shrinking: the sheet folds first; the window follows after the delay.
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenCalledWith(NARROW_WINDOW);
  });

  it("travels while Juno works, leaves a tick per finished tool, and pauses at the running one", async () => {
    const { rerender } = render(<HaloBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "How long until my meeting" });
    expect(verb()).toBe("travel");
    expect(ring().getAttribute("data-hold")).toBeNull();
    expect(screen.getByTestId("halo-caption")).toHaveTextContent("How long until my meeting");

    chat.messages = [
      user("How long until my meeting"),
      tool("Checking your calendar", { success: true }),
      tool("Reading the invite", { success: true }),
      tool("Opening Safari"),
    ];
    rerender(<HaloBar />);
    expect(screen.getAllByTestId("halo-tick")).toHaveLength(2);
    expect(ring().getAttribute("data-hold")).toBe(String(tickAngle(2)));
    expect(screen.getByTestId("halo-caption")).toHaveTextContent("Opening Safari");
  });

  it("holds a short answer inside the ring once it is complete, not while it streams", async () => {
    const { rerender } = render(<HaloBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "How long" });
    chat.messages = [user("How long"), { role: "assistant", content: "42 min", messageId: "m1", isStreaming: true }];
    rerender(<HaloBar />);
    expect(screen.queryByTestId("halo-inside")).toBeNull();
    expect(screen.queryByTestId("halo-panel")).toBeNull();

    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-inside")).toHaveTextContent("42 min");
    expect(screen.queryByTestId("halo-panel")).toBeNull();
    expect(sheet()).toBe("closed");
  });

  it("unfolds a long answer below the ring while it streams, then drains the ring and settles", async () => {
    const { rerender } = render(<HaloBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Q" });
    chat.messages = [
      user("Q"),
      { role: "assistant", content: "The meeting moved to Friday at three.", messageId: "m1", isStreaming: true },
    ];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-panel")).toBeInTheDocument();
    expect(screen.getByTestId("answer")).toHaveTextContent("The meeting moved to Friday at three.");
    expect(sheet()).toBe("open");
    // Still streaming, still working: the ring travels, no clock yet.
    expect(verb()).toBe("travel");

    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<HaloBar />);
    await send(UI.BAR_STATES_FINISHING);
    expect(verb()).toBe("close");
    await send(UI.BAR_STATES_DEFAULT);
    // The close holds past Rust's move to Default.
    expect(verb()).toBe("close");
    await act(async () => {
      vi.advanceTimersByTime(CLOSE_MS + 10);
    });
    expect(verb()).toBe("drain");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(screen.queryByTestId("halo-panel")).toBeNull();
    expect(verb()).toBe("rest");
    expect(sheet()).toBe("closed");
  });

  it("holds the clock while the pointer is over it, and Escape dismisses at once", async () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [user("Q"), { role: "assistant", content: "Sunny, 72.", messageId: "m1", timestamp: 2 }];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-inside")).toHaveTextContent("Sunny, 72.");
    expect(verb()).toBe("drain");
    fireEvent.pointerEnter(screen.getByTestId("halo"));
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(screen.getByTestId("halo-inside")).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByTestId("halo-inside")).toBeNull();
    // Idle and nothing working: Escape dismissed locally, it did not stop anything.
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("does not show an answer that was already there when it mounted", () => {
    chat.messages = [user("Q"), { role: "assistant", content: "Old", messageId: "old", timestamp: 2 }];
    render(<HaloBar />);
    expect(screen.queryByTestId("halo-inside")).toBeNull();
    expect(verb()).toBe("rest");
  });

  it("clears the answer when you speak again", async () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [user("Q"), { role: "assistant", content: "A", messageId: "m1", timestamp: 2 }];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-inside")).toBeInTheDocument();
    await send(UI.BAR_STATES_LISTENING);
    expect(screen.queryByTestId("halo-inside")).toBeNull();
    expect(verb()).toBe("fill");
  });

  it("pauses for a tool waiting on you, asks inside the ring, and tells the backend what you decided", async () => {
    const { rerender } = render(<HaloBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Open it" });
    chat.messages = [
      user("Open it"),
      tool("open the invite in Safari", { tool_name: "browser", tool_id: "t1", approval_state: "pending" }),
    ];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-inside")).toHaveTextContent("Allow?");
    expect(ring().getAttribute("data-hold")).toBe("0");
    expect(screen.getByTestId("halo-approval")).toHaveTextContent("Juno wants to open the invite in Safari");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("breaks red where it failed and says what went wrong", async () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [user("Q"), tool("Checking your calendar", { success: true })];
    rerender(<HaloBar />);
    await send(UI.BAR_STATES_ERROR, { currentError: "Calendar did not answer" });
    expect(verb()).toBe("gap");
    expect(screen.getByTestId("halo-gap")).toBeInTheDocument();
    expect(screen.getAllByTestId("halo-tick")).toHaveLength(1);
    const caption = screen.getByTestId("halo-caption");
    expect(caption).toHaveTextContent("Calendar did not answer");
    expect(caption.getAttribute("data-tone")).toBe("error");
  });

  it("pulses while Juno speaks and keeps the answer where it was", async () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [user("Q"), { role: "assistant", content: "Yes", messageId: "m1", timestamp: 2 }];
    rerender(<HaloBar />);
    await send(UI.BAR_STATES_SPEAKING, { spokenText: "Yes" });
    expect(verb()).toBe("pulse");
    expect(screen.getByTestId("halo-inside")).toHaveTextContent("Yes");
    expect(screen.queryByTestId("halo-caption")).toBeNull();
  });

  it("shows a spoken-only reply as the words themselves", () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [
      user("Q"),
      {
        role: "assistant",
        content: "",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Done."], total_spoken_text: "Done." },
      },
    ];
    rerender(<HaloBar />);
    expect(screen.getByTestId("halo-inside")).toHaveTextContent("Done.");
  });

  it("keeps the spoken channel behind one button under a long answer", () => {
    const { rerender } = render(<HaloBar />);
    chat.messages = [
      user("Q"),
      {
        role: "assistant",
        content: "Here is the list of everything that changed today.",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Here you go."], total_spoken_text: "Here you go." },
      },
    ];
    rerender(<HaloBar />);
    expect(screen.queryByTestId("halo-spoken")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Spoken aloud" }));
    expect(screen.getByTestId("halo-spoken")).toHaveTextContent("Here you go.");
  });

  it("Escape while Juno works stops the work instead of dismissing", async () => {
    render(<HaloBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_ESCAPE }),
      }),
    );
  });

  it("opens a composer under the ring to type in, and submits on return", async () => {
    render(<HaloBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(sheet()).toBe("open");
    const input = screen.getByRole("textbox", { name: "Ask Juno" });
    fireEvent.change(input, { target: { value: "Send it" } });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({
          interaction_type: UI.INTERACTION_TYPES_INPUT_CHANGE,
          data: { value: "Send it" },
        }),
      }),
    );
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({
          interaction_type: UI.INTERACTION_TYPES_SUBMIT,
          data: { value: "Send it" },
        }),
      }),
    );
  });

  it("a click on the idle ring asks Rust to open the composer", () => {
    render(<HaloBar />);
    fireEvent.click(screen.getByRole("button", { name: "Ask Juno" }));
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_CLICK }),
      }),
    );
  });

  it("travels in blue and says so while Juno drives the cursor", async () => {
    render(<HaloBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, tool: "computer", target_app: "Safari" });
    });
    expect(verb()).toBe("travel");
    expect(screen.getByTestId("halo-caption")).toHaveTextContent("using the mouse in Safari");
  });
});
