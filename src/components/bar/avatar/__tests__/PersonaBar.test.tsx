import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { HEAD, HEAD_ANCHOR, LINGER_MS, PAD, PANEL_WIDTH, SHRINK_DELAY_MS } from "../avatarModel";

// ── Tauri, Rive and hook stand-ins ──────────────────────────────────

const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged, chat, monitorY } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(true)),
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  resizeWindowIfChanged: vi.fn((_size: { width: number; height: number }) => Promise.resolve()),
  chat: {
    messages: [] as ChatMessage[],
    isProcessing: false,
    handleApprovalUpdate: vi.fn(),
  },
  // Where the fake window sits: y of its top edge, on a 900px display.
  monitorY: { top: 40 },
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
    outerPosition: vi.fn(async () => ({ x: 1000, y: monitorY.top })),
    outerSize: vi.fn(async () => ({ width: 116, height: 116 })),
  }),
  availableMonitors: vi.fn(async () => [{ position: { x: 0, y: 0 }, size: { width: 1440, height: 900 } }]),
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
// The Rive sphere needs WebGL and the network. The head only needs to know
// which input it would set.
vi.mock("@/components/ai-elements/persona", () => ({
  Persona: ({ state, onLoad }: { state: string; onLoad?: () => void }) => {
    void onLoad;
    return <div data-testid="rive" data-state={state} />;
  },
}));
vi.mock("motion/react", () => ({ useReducedMotion: () => true }));

import { PersonaBar } from "../../persona-bar";

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

/** Let the window protocol's awaited resize settle. */
async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

const gesture = () => screen.getByTestId("avatar-head").getAttribute("data-gesture");
const rive = () => screen.getByTestId("rive").getAttribute("data-state");
const isOpen = () => screen.getByTestId("avatar-root").getAttribute("data-open") === "true";

const REST = { width: HEAD + 2 * PAD, height: HEAD + 2 * PAD, anchorY: HEAD_ANCHOR };

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  chat.messages = [];
  chat.isProcessing = false;
  monitorY.top = 40;
});

afterEach(() => {
  vi.useRealTimers();
});

describe("PersonaBar", () => {
  it("rests as a calm head with nothing in Rive, sized to the head alone", async () => {
    render(<PersonaBar />);
    await settle();
    expect(gesture()).toBe("calm");
    expect(rive()).toBe("idle");
    expect(isOpen()).toBe(false);
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ ...REST, growUp: false });
  });

  it("the cue by its ear swells with your voice while it listens", async () => {
    render(<PersonaBar />);
    await settle();
    const swell = () => Number(screen.getByTestId("avatar-cue-swell").getAttribute("data-swell"));
    await send(UI.BAR_STATES_LISTENING, { audioLevel: 0 });
    expect(swell()).toBe(1);
    await send(UI.BAR_STATES_LISTENING, { audioLevel: 0.64 });
    expect(swell()).toBeGreaterThan(1.5);
    await send(UI.BAR_STATES_LISTENING, { audioLevel: 0.1 });
    expect(swell()).toBeLessThan(1.5);
  });

  it("leans in with a blue cue and your words in a bubble on your side as you speak", async () => {
    render(<PersonaBar />);
    await settle();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft to Maya" });
    expect(gesture()).toBe("lean");
    expect(rive()).toBe("listening");
    expect(screen.getByTestId("avatar-cue").getAttribute("data-motion")).toBe("breathe");
    const yours = screen.getByTestId("avatar-yours");
    expect(yours).toHaveTextContent("Send the draft to Maya");
    expect(yours.getAttribute("data-tail")).toBe("you");
    expect(yours.getAttribute("data-edge")).toBe("listen");
  });

  it("tints your bubble green while dictating", async () => {
    render(<PersonaBar />);
    await send(UI.BAR_STATES_DICTATING, { transcriptionText: "Dear Maya" });
    expect(screen.getByTestId("avatar-yours").getAttribute("data-edge")).toBe("dictation");
    expect(screen.getByTestId("avatar-yours").getAttribute("data-tone")).toBe("dictation");
  });

  it("grows the window before it shows a bubble, and shrinks it after the bubble has left", async () => {
    render(<PersonaBar />);
    await settle();
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Hi" });
    // The bubble is laid out but hidden until the window has grown.
    expect(resizeWindowIfChanged).toHaveBeenCalledTimes(1);
    expect(resizeWindowIfChanged.mock.calls[0][0]).toMatchObject({ width: PANEL_WIDTH, anchorY: HEAD_ANCHOR });
    await settle();
    expect(screen.getByTestId("avatar-yours").getAttribute("data-shown")).toBe("true");

    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_DEFAULT);
    // Gone from the scene at once; the window follows after the exit.
    expect(screen.queryByTestId("avatar-yours")).toBeNull();
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ ...REST, growUp: false });
  });

  it("keeps the head's anchor the same in every posture, so it never moves on screen", async () => {
    render(<PersonaBar />);
    await settle();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Hi" });
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Hi" });
    for (const call of resizeWindowIfChanged.mock.calls) {
      expect(call[0]).toMatchObject({ anchorY: HEAD_ANCHOR });
    }
  });

  it("raises the bubbles above the head when the window sits in the bottom half of the display", async () => {
    monitorY.top = 700;
    render(<PersonaBar />);
    await settle();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Hi" });
    await settle();
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith(expect.objectContaining({ growUp: true }));
    expect(screen.getByTestId("avatar-root").getAttribute("data-facing-up")).toBe("true");
  });

  it("thinks in a thought bubble with the running tool named inside", async () => {
    const { rerender } = render(<PersonaBar />);
    await send(UI.BAR_STATES_SUBMITTING, { lastSubmittedValue: "Open the page" });
    expect(gesture()).toBe("think");
    expect(rive()).toBe("thinking");
    expect(screen.getByTestId("avatar-thought")).toHaveTextContent("Thinking");
    expect(screen.getByTestId("avatar-thinking-mark")).toBeInTheDocument();
    // What you asked stays on your side, dimmed.
    expect(screen.getByTestId("avatar-yours")).toHaveTextContent("Open the page");
    expect(screen.getByTestId("avatar-yours").getAttribute("data-tone")).toBe("dim");

    chat.messages = [
      { role: "user", content: "Open the page", timestamp: 1 },
      { role: "tool_call_request", content: "Reading the page", tool_name: "browser", tool_id: "t1" },
    ];
    rerender(<PersonaBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Open the page" });
    expect(screen.getByTestId("avatar-thought")).toHaveTextContent("Reading the page");
  });

  it("talks: the answer lands in Juno's bubble, spoken part first, notes beneath", async () => {
    const { rerender } = render(<PersonaBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "What is the weather" });
    expect(screen.getByTestId("avatar-thought")).toBeInTheDocument();
    chat.messages = [
      { role: "user", content: "What is the weather", timestamp: 1 },
      {
        role: "assistant",
        content: "Sunny, 72. <WeatherCard />",
        messageId: "m1",
        isStreaming: true,
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Sunny and warm."], total_spoken_text: "Sunny and warm." },
      },
    ];
    rerender(<PersonaBar />);
    expect(screen.queryByTestId("avatar-thought")).toBeNull();
    const bubble = screen.getByTestId("avatar-answer");
    expect(bubble.getAttribute("data-tail")).toBe("head");
    expect(gesture()).toBe("talk");
    expect(rive()).toBe("speaking");
    expect(screen.getByTestId("avatar-spoken")).toHaveTextContent("Sunny and warm.");
    expect(screen.getByTestId("answer")).toHaveTextContent("Sunny, 72.");
    // Still streaming: the linger does not count.
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
  });

  it("folds the spoken sentence away behind the speaker control", async () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "Here is the list.",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Here you go."], total_spoken_text: "Here you go." },
      },
    ];
    rerender(<PersonaBar />);
    expect(screen.getByTestId("avatar-spoken")).toHaveTextContent("Here you go.");
    fireEvent.click(screen.getByRole("button", { name: "Spoken aloud" }));
    expect(screen.queryByTestId("avatar-spoken")).toBeNull();
    expect(screen.getByTestId("answer")).toHaveTextContent("Here is the list.");
  });

  it("does not read the spoken sentence twice when the notes open with it", () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "Done. The draft is with Maya.\n\nI will let you know when she replies.",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: {
          has_spoken_content: true,
          tts_parts: ["Done. The draft is with Maya."],
          total_spoken_text: "Done. The draft is with Maya.",
        },
      },
    ];
    rerender(<PersonaBar />);
    expect(screen.queryByTestId("avatar-spoken")).toBeNull();
    expect(screen.queryByRole("button", { name: "Spoken aloud" })).toBeNull();
    expect(screen.getByTestId("answer")).toHaveTextContent("Done. The draft is with Maya.");
  });

  it("makes a spoken-only reply the body itself, with nothing to fold", () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["All set."], total_spoken_text: "All set." },
      },
    ];
    rerender(<PersonaBar />);
    expect(screen.getByTestId("avatar-spoken")).toHaveTextContent("All set.");
    expect(screen.queryByTestId("avatar-notes")).toBeNull();
    expect(screen.queryByRole("button", { name: "Spoken aloud" })).toBeNull();
  });

  it("nods when finished and lets the answer go once the linger runs out", async () => {
    const { rerender } = render(<PersonaBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Q" });
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", isStreaming: false, timestamp: 2 },
    ];
    rerender(<PersonaBar />);
    await send(UI.BAR_STATES_FINISHING, { lastSubmittedValue: "Q" });
    expect(gesture()).toBe("nod");
    expect(screen.getByTestId("avatar-answer")).toBeInTheDocument();
    await send(UI.BAR_STATES_DEFAULT);
    expect(gesture()).toBe("calm");
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(screen.queryByTestId("avatar-answer")).toBeNull();
    expect(isOpen()).toBe(false);
  });

  it("holds the answer while the pointer is over it, and Escape lets it go at once", async () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<PersonaBar />);
    expect(screen.getByTestId("avatar-answer")).toBeInTheDocument();
    fireEvent.pointerEnter(screen.getByTestId("avatar-root"));
    expect(screen.getByTestId("island-linger").getAttribute("data-paused")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(screen.getByTestId("avatar-answer")).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByTestId("avatar-answer")).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("does not raise an answer that was already there when it mounted", () => {
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Old", messageId: "old", timestamp: 2 },
    ];
    render(<PersonaBar />);
    expect(screen.queryByTestId("avatar-answer")).toBeNull();
  });

  it("lets the last answer go when you speak again", async () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<PersonaBar />);
    expect(screen.getByTestId("avatar-answer")).toBeInTheDocument();
    await send(UI.BAR_STATES_LISTENING);
    expect(screen.queryByTestId("avatar-answer")).toBeNull();
    expect(screen.getByTestId("avatar-yours")).toHaveTextContent("Listening");
  });

  it("holds up the question when a tool needs you, wide-eyed, and tells the backend what you decided", async () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Open it", timestamp: 1 },
      {
        role: "tool_call_request",
        content: "open the page in Safari",
        tool_name: "browser",
        tool_id: "t1",
        approval_state: "pending",
      },
    ];
    rerender(<PersonaBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Open it" });
    expect(gesture()).toBe("wide");
    expect(screen.getByTestId("avatar-ask")).toHaveTextContent("Can I open the page in Safari?");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("winces at a failure and says what went wrong in a red-edged bubble", async () => {
    render(<PersonaBar />);
    await send(UI.BAR_STATES_ERROR, { currentError: "Connection unavailable", lastSubmittedValue: "Q" });
    expect(gesture()).toBe("wince");
    const bubble = screen.getByTestId("avatar-error");
    expect(bubble).toHaveTextContent("Connection unavailable");
    expect(bubble.getAttribute("data-edge")).toBe("error");
    expect(bubble.getAttribute("role")).toBe("alert");
  });

  it("says so while Juno drives the cursor", async () => {
    render(<PersonaBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, target_app: "Safari" });
    });
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Q" });
    expect(screen.getByTestId("avatar-thought")).toHaveTextContent("Juno is using the mouse in Safari");
    expect(rive()).toBe("thinking");
  });

  it("Escape while Juno works stops the work instead of closing", async () => {
    render(<PersonaBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_ESCAPE }),
      }),
    );
  });

  it("opens the composer in your bubble on a click at rest, types, and submits on return", async () => {
    render(<PersonaBar />);
    fireEvent.click(screen.getByRole("button", { name: "Ask Juno" }));
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        elementId: "floating-bar",
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_CLICK }),
      }),
    );
    await send(UI.BAR_STATES_INPUT);
    expect(gesture()).toBe("attend");
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

  it("settles to rest, not the composer, when an answer is dismissed under focus", async () => {
    const { rerender } = render(<PersonaBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<PersonaBar />);
    await send(UI.BAR_STATES_INPUT);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_BLUR }),
      }),
    );
  });
});
