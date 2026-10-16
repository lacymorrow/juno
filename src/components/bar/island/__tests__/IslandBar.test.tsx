import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { LINGER_MS } from "../islandModel";
import { LEAVE_VERIFY_MS } from "../useIslandHover";
import { PILL_TOOLTIP_DELAY_MS } from "../../PillTooltip";

// ── Tauri and hook stand-ins ────────────────────────────────────────

const { invoke, emit, listenHandlers, eventHandlers, chat, focusHandlers, setFocus, frameReport, agents, backend } = vi.hoisted(() => ({
  agents: { sessions: [] as Array<Record<string, unknown>>, focusSession: vi.fn() },
  /** What the stand-in backend answers, per command. Unlisted commands answer true. */
  backend: { answers: {} as Record<string, unknown> },
  focusHandlers: [] as Array<(event: { payload: boolean }) => void>,
  setFocus: vi.fn(async () => {}),
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(true)),
  emit: vi.fn(async () => {}),
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  frameReport: vi.fn(),
  chat: {
    messages: [] as ChatMessage[],
    isProcessing: false,
    handleApprovalUpdate: vi.fn(),
    stop: vi.fn(async () => {}),
  },
}));

vi.mock("@/hooks/useAgentSessions", () => ({
  useAgentSessions: () => ({ sessions: agents.sessions, focusSession: agents.focusSession, cancelSession: vi.fn() }),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    listenHandlers.set(event, handler);
    return () => listenHandlers.delete(event);
  }),
  emit,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    onFocusChanged: vi.fn(async (handler: (event: { payload: boolean }) => void) => {
      focusHandlers.push(handler);
      return () => {};
    }),
    startDragging: vi.fn(async () => {}),
    setFocus,
    outerPosition: async () => ({ x: 0, y: 0 }),
    outerSize: async () => ({ width: 200, height: 100 }),
    scaleFactor: async () => 1,
    setPosition: vi.fn(async () => {}),
  }),
  // The cursor is parked outside the island, so a verified leave is believed.
  cursorPosition: async () => ({ x: 9000, y: 9000 }),
  availableMonitors: async () => [],
  PhysicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
}));
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    eventHandlers.set(event, handler);
  },
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
vi.mock("@/components/chat/ChatContainerV2", () => ({
  ChatContainerV2: ({ conversation }: { conversation: ChatMessage[] }) => (
    <div data-testid="chat-history">{conversation.map((m) => m.content).join(" | ")}</div>
  ),
}));
vi.mock("@/components/input-control/InputControlNotices", () => ({
  InputControlNotices: () => null,
}));
vi.mock("@/components/ui/mixed-content-renderer", () => ({
  MixedContentRenderer: ({ content }: { content: string }) => <div data-testid="answer">{content}</div>,
  // The real splitter is tested with the renderer; a component here is one self-closing tag.
  splitHeroContent: (content: string) => {
    const m = content.match(/<([A-Z]\w*)[^>]*\/>/);
    if (!m) return null;
    return { components: m[0], prose: content.replace(m[0], " ").replace(/\s+/g, " ").trim() };
  },
}));
// The steady frame, already placed in a top-centre well: the island grows down.
vi.mock("../useIslandFrame", async () => {
  const { steadyLayout } = await import("@/lib/steadyFrame");
  const { ISLAND_STEADY_SPEC } = await import("../islandModel");
  const layout = steadyLayout(
    { x: 650, y: 30, width: 140, height: 76, fx: 0.5, fy: 0, monitorIndex: 0 },
    { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 },
    ISLAND_STEADY_SPEC,
  );
  return {
    useIslandFrame: () => ({ layout, hidden: false, region: { current: null }, report: frameReport }),
  };
});
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
  return {
    useReducedMotion: () => true,
    AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
    motion: {
      div: React.forwardRef<HTMLDivElement, Record<string, unknown>>((props, ref) =>
        React.createElement("div", { ref, ...strip(props) }),
      ),
    },
  };
});

import { IslandBar } from "../IslandBar";

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

const face = () => screen.getByTestId("island-shell").getAttribute("data-face");
const composer = () => screen.getByTestId("island-composer").querySelector("textarea") as HTMLTextAreaElement;
const placeholder = () => screen.getByTestId("island-placeholder");
const stage = () => screen.getByTestId("island-stage");

const sentInteraction = (type: string) =>
  invoke.mock.calls.some(
    ([cmd, args]) =>
      cmd === "ui_handle_interaction" &&
      (args as { interaction?: { interaction_type?: string } })?.interaction?.interaction_type === type,
  );

/** A pointer leave is believed only once the cursor is checked, a moment later. */
async function leaveVerified(el: HTMLElement) {
  fireEvent.pointerLeave(el);
  await act(async () => {
    vi.advanceTimersByTime(LEAVE_VERIFY_MS + 10);
  });
  await act(async () => {});
}

const ANSWER: ChatMessage[] = [
  { role: "user", content: "What is the weather", timestamp: 1 },
  { role: "assistant", content: "Sunny, 72.", messageId: "m1", timestamp: 2 },
];

const COMPONENT_ANSWER: ChatMessage[] = [
  { role: "user", content: "Send the draft to Maya", timestamp: 1 },
  {
    role: "assistant",
    content: 'Done. The draft is with Maya. <TaskSummaryCard title="Sent" /> I will let you know.',
    messageId: "c1",
    timestamp: 2,
  },
];

/** Render, then let an answer arrive (so it is new, not history). */
function renderWithAnswer(messages: ChatMessage[] = ANSWER) {
  const view = render(<IslandBar />);
  chat.messages = messages;
  view.rerender(<IslandBar />);
  return view;
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  invoke.mockClear();
  backend.answers = { get_connectivity: null, get_triggers: [], get_always_listening_status: false };
  invoke.mockImplementation((cmd: unknown) =>
    Promise.resolve(typeof cmd === "string" && cmd in backend.answers ? backend.answers[cmd] : true),
  );
  agents.sessions = [];
  emit.mockClear();
  frameReport.mockClear();
  chat.messages = [];
  chat.isProcessing = false;
  chat.stop.mockClear();
  focusHandlers.length = 0;
  setFocus.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("IslandBar: one surface", () => {
  it("rests as the capsule, and past rest it is the text box, saying what is happening", async () => {
    render(<IslandBar />);
    expect(face()).toBe("idle");
    await send(UI.BAR_STATES_LISTENING);
    expect(face()).toBe("compose");
    expect(placeholder()).toHaveTextContent("Listening…");
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft to Maya" });
    expect(placeholder()).toHaveTextContent("Send the draft to Maya");
    await send(UI.BAR_STATES_TRANSCRIBING);
    expect(face()).toBe("compose");
    await send(UI.BAR_STATES_SUBMITTING, { lastSubmittedValue: "Send it" });
    expect(placeholder()).toHaveTextContent("Sending…");
    await send(UI.BAR_STATES_LOADING);
    expect(placeholder()).toHaveTextContent("Working…");
    await send(UI.BAR_STATES_ERROR, { currentError: "Connection unavailable" });
    expect(face()).toBe("compose");
    expect(placeholder()).toHaveTextContent("Connection unavailable");
    await send(UI.BAR_STATES_DEFAULT);
    expect(face()).toBe("idle");
  });

  it("keeps one composer element through every state", async () => {
    render(<IslandBar />);
    const node = composer();
    for (const state of [
      UI.BAR_STATES_INPUT,
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_DEFAULT,
    ]) {
      await send(state);
      expect(composer()).toBe(node);
    }
  });

  it.each([
    [UI.BAR_STATES_LISTENING, {}],
    [UI.BAR_STATES_TRANSCRIBING, { transcriptionText: "testing", transcriptionProvisional: true }],
  ])("while a turn records: level bars for the mic, Send what was said, Cancel (%s)", async (state, extra) => {
    render(<IslandBar />);
    await send(state, { audioLevel: 0.5, ...extra });
    expect(screen.getByTestId("island-level")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Talk to Juno" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "stop" });
    fireEvent.click(screen.getByRole("button", { name: "Cancel without sending" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "cancel" });
  });

  it("once the mic has closed, the small bar has nothing to send until you type", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_TRANSCRIBING, { transcriptionProvisional: false });
    expect(screen.queryByTestId("island-level")).toBeNull();
    expect(screen.getByTestId("island-composer")).toHaveAttribute("data-compact", "true");
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    // The caret in the box makes it the full text box, with Send for the typed line.
    fireEvent.focus(composer());
    expect(screen.getByTestId("island-composer")).not.toHaveAttribute("data-compact");
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("the stop control stops Juno while it works", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.click(screen.getByRole("button", { name: "Stop Juno" }));
    expect(chat.stop).toHaveBeenCalled();
  });

  it("types, sends on Enter and keeps the caret for the next line", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(face()).toBe("compose");
    expect(document.activeElement).toBe(composer());
    fireEvent.change(composer(), { target: { value: "Send it" } });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({
          interaction_type: UI.INTERACTION_TYPES_INPUT_CHANGE,
          data: { value: "Send it" },
        }),
      }),
    );
    fireEvent.keyDown(composer(), { key: "Enter" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({
          interaction_type: UI.INTERACTION_TYPES_SUBMIT,
          data: { value: "Send it" },
        }),
      }),
    );
    expect(composer().value).toBe("");
  });

  it("never clears a draft on a state change", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    fireEvent.change(composer(), { target: { value: "half a thought" } });
    await send(UI.BAR_STATES_SHRINKING);
    await send(UI.BAR_STATES_DEFAULT);
    expect(composer().value).toBe("half a thought");
    // A draft at rest keeps the box up.
    expect(face()).toBe("compose");
  });

  it("reports the drawn shape as the only part that takes the mouse", async () => {
    render(<IslandBar />);
    const last = () => frameReport.mock.calls[frameReport.mock.calls.length - 1];
    expect(last()[1][0]).toMatchObject({ width: 92, height: 28 });
    await send(UI.BAR_STATES_INPUT);
    expect(last()[1][0]).toMatchObject({ width: 360, height: 36 });
    // The docked top edge never moves: idle and compose start on the same line.
    const hits = frameReport.mock.calls.map((c) => (c[1] as Array<{ y: number }>)[0].y);
    expect(new Set(hits).size).toBe(1);
  });
});

describe("IslandBar: answer", () => {
  it("streams the answer above the composer, then settles back once the ring runs out", async () => {
    const view = render(<IslandBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "What is the weather" });
    chat.messages = [ANSWER[0], { ...ANSWER[1], isStreaming: true }];
    view.rerender(<IslandBar />);
    expect(face()).toBe("answer");
    expect(screen.getByTestId("island-question")).toHaveTextContent("What is the weather");
    expect(screen.getByTestId("answer")).toHaveTextContent("Sunny, 72.");
    expect(composer()).toHaveAccessibleName("Follow up");
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");

    chat.messages = ANSWER;
    view.rerender(<IslandBar />);
    await send(UI.BAR_STATES_DEFAULT);
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(face()).toBe("idle");
  });

  it("the composer is the same element in compose and answer: a follow-up never changes fields", async () => {
    const view = render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    const node = composer();
    chat.messages = ANSWER;
    view.rerender(<IslandBar />);
    expect(face()).toBe("answer");
    expect(composer()).toBe(node);
  });

  it("holds the ring under the pointer, and while the composer has the caret", async () => {
    renderWithAnswer();
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");
    fireEvent.pointerEnter(stage());
    expect(screen.getByTestId("island-linger").getAttribute("data-paused")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(face()).toBe("answer");
    act(() => composer().focus());
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
  });

  it("Escape puts an answer away at once, and tells Rust when the follow-up field had it in input", async () => {
    renderWithAnswer();
    await send(UI.BAR_STATES_INPUT);
    expect(face()).toBe("answer");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("idle");
    expect(sentInteraction(UI.INTERACTION_TYPES_BLUR)).toBe(true);
  });

  it("does not open for an answer that was already there when it mounted", () => {
    chat.messages = ANSWER;
    render(<IslandBar />);
    expect(face()).toBe("idle");
  });

  it("a spoken turn turns the answer back into the composer, with the draft kept", async () => {
    renderWithAnswer();
    fireEvent.change(composer(), { target: { value: "and tomorrow" } });
    await send(UI.BAR_STATES_LISTENING);
    expect(face()).toBe("compose");
    expect(composer().value).toBe("and tomorrow");
    // Cancelled: the answer is still there to read.
    await send(UI.BAR_STATES_DEFAULT);
    expect(face()).toBe("answer");
  });

  it("LAC-4240: a dictation into the follow-up field keeps its field, its caret and the draft", async () => {
    renderWithAnswer();
    // Clicking the follow-up field: the caret lands, Rust moves to input.
    act(() => composer().focus());
    await send(UI.BAR_STATES_INPUT);
    fireEvent.change(composer(), { target: { value: "Also ask" } });
    const node = composer();

    // Rust's real key-up sequence (`handle_dictation_stop`), rest blip included.
    await send(UI.BAR_STATES_DICTATING, { isDictationMode: true, audioLevel: 0.4 });
    await send(UI.BAR_STATES_LISTENING, { isDictationMode: true });
    await send(UI.BAR_STATES_TRANSCRIBING, {
      isDictationMode: true,
      transcriptionProvisional: true,
      transcriptionText: "about Friday",
    });
    await send(UI.BAR_STATES_DEFAULT);
    await send(UI.BAR_STATES_TRANSCRIBING, { transcriptionProvisional: false });
    await send(UI.BAR_STATES_DEFAULT);

    expect(composer()).toBe(node);
    expect(node.isConnected).toBe(true);
    expect(document.activeElement).toBe(node);
    expect(node.value).toBe("Also ask");
    expect(face()).toBe("answer");
    // The paste lands in the focused field.
    fireEvent.change(node, { target: { value: "Also ask about Friday" } });
    expect(composer().value).toBe("Also ask about Friday");
  });

  it("shows a tool waiting on you as one row, and tells the backend what you decided", async () => {
    const view = render(<IslandBar />);
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
    view.rerender(<IslandBar />);
    expect(face()).toBe("answer");
    expect(screen.getByTestId("island-approval")).toHaveTextContent("Juno wants to open the page in Safari");
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("keeps the spoken channel behind one button, and a pressed button restarts the ring on leave", async () => {
    renderWithAnswer([
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "Here is the list.",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Here you go."], total_spoken_text: "Here you go." },
      },
    ]);
    expect(screen.queryByTestId("island-spoken")).toBeNull();
    fireEvent.pointerEnter(stage());
    fireEvent.click(screen.getByRole("button", { name: "Spoken aloud" }));
    expect(screen.getByTestId("island-spoken")).toHaveTextContent("Here you go.");
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
    await leaveVerified(stage());
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");
  });

  it("shows a spoken-only reply as the body itself", () => {
    renderWithAnswer([
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Done."], total_spoken_text: "Done." },
      },
    ]);
    expect(screen.getByTestId("island-spoken-only")).toHaveTextContent("Done.");
    expect(screen.queryByRole("button", { name: "Spoken aloud" })).toBeNull();
  });

  it("Escape while Juno works stops the work and keeps the draft", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.change(composer(), { target: { value: "next" } });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(sentInteraction(UI.INTERACTION_TYPES_ESCAPE)).toBe(true);
    expect(composer().value).toBe("next");
  });
});

describe("IslandBar: component", () => {
  it("makes the component the hero, with the prose folded into one caption line", async () => {
    renderWithAnswer(COMPONENT_ANSWER);
    expect(face()).toBe("component");
    expect(screen.getByTestId("island-hero")).toHaveTextContent('<TaskSummaryCard title="Sent" />');
    expect(screen.getByTestId("island-caption")).toHaveTextContent("Done. The draft is with Maya. I will let you know.");
    expect(screen.queryByTestId("island-caption-full")).toBeNull();
    fireEvent.click(screen.getByTestId("island-caption"));
    expect(screen.getByTestId("island-caption-full")).toBeInTheDocument();
  });

  it("X steps back to the composer, with the caret in it", async () => {
    renderWithAnswer(COMPONENT_ANSWER);
    const node = composer();
    fireEvent.click(screen.getByTestId("island-linger"));
    expect(face()).toBe("compose");
    expect(composer()).toBe(node);
    expect(document.activeElement).toBe(node);
  });

  it("Escape steps down one rung: component to composer, composer to idle", async () => {
    renderWithAnswer(COMPONENT_ANSWER);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("compose");
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("idle");
  });

  it("settles to idle when its ring runs out", async () => {
    renderWithAnswer(COMPONENT_ANSWER);
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(face()).toBe("idle");
  });
});

describe("IslandBar: idle and hover", () => {
  it("shows its controls under the pointer and puts them away when it leaves", async () => {
    render(<IslandBar />);
    fireEvent.pointerEnter(stage());
    expect(face()).toBe("hover");
    expect(screen.getByRole("button", { name: "Talk to Juno" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Type to Juno" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show last answer" })).toBeNull();
    await leaveVerified(stage());
    expect(face()).toBe("idle");
  });

  it("answers the native tracking area too, which is what fires while another app is active", async () => {
    render(<IslandBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.SYSTEM_MOUSE_ENTERED_WINDOW)?.(null);
    });
    expect(face()).toBe("hover");
    await act(async () => {
      eventHandlers.get(EVENTS.SYSTEM_MOUSE_LEFT_WINDOW)?.(null);
      vi.advanceTimersByTime(LEAVE_VERIFY_MS + 10);
    });
    await act(async () => {});
    expect(face()).toBe("idle");
  });

  it("hover controls talk and type without the click also opening the composer twice", async () => {
    render(<IslandBar />);
    fireEvent.pointerEnter(stage());
    invoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Talk to Juno" }));
    expect(invoke).toHaveBeenCalledWith("agent_voice", { action: "start" });
    expect(sentInteraction(UI.INTERACTION_TYPES_CLICK)).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Type to Juno" }));
    expect(setFocus).toHaveBeenCalled();
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ui_handle_interaction")).toHaveLength(1);
    expect(sentInteraction(UI.INTERACTION_TYPES_CLICK)).toBe(true);
  });

  it("brings a settled answer back from the hover controls, and the tray toggles the chat face", async () => {
    renderWithAnswer();
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(face()).toBe("idle");
    fireEvent.pointerEnter(stage());
    fireEvent.click(screen.getByRole("button", { name: "Show last answer" }));
    expect(face()).toBe("answer");
    // Show Chat: the whole conversation; again, back to the capsule.
    await act(async () => {
      eventHandlers.get(EVENTS.BAR_TOGGLE_PANE)?.(null);
    });
    expect(face()).toBe("chat");
    await act(async () => {
      eventHandlers.get(EVENTS.BAR_TOGGLE_PANE)?.(null);
    });
    expect(face()).toBe("idle");
  });

  it("Escape under the pointer settles all the way to the capsule", async () => {
    renderWithAnswer();
    fireEvent.pointerEnter(stage());
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("idle");
    await leaveVerified(stage());
    fireEvent.pointerEnter(stage());
    expect(face()).toBe("hover");
  });

  it("drags through a hover control without pressing it", async () => {
    render(<IslandBar />);
    fireEvent.pointerEnter(stage());
    const mic = screen.getByRole("button", { name: "Talk to Juno" });
    invoke.mockClear();
    fireEvent.mouseDown(mic, { button: 0, clientX: 40, clientY: 40 });
    fireEvent.mouseMove(mic, { clientX: 70, clientY: 50 });
    fireEvent.mouseUp(mic);
    fireEvent.click(mic);
    expect(invoke).not.toHaveBeenCalledWith("agent_voice", expect.anything());
    expect(face()).toBe("hover");
  });

  it("never tells Rust the window gained focus, so grabbing it to drag cannot open the composer", async () => {
    render(<IslandBar />);
    await act(async () => {});
    expect(focusHandlers.length).toBeGreaterThan(0);
    invoke.mockClear();
    await act(async () => {
      focusHandlers.forEach((h) => h({ payload: true }));
    });
    expect(sentInteraction(UI.INTERACTION_TYPES_FOCUS)).toBe(false);
    await act(async () => {
      focusHandlers.forEach((h) => h({ payload: false }));
    });
    expect(sentInteraction(UI.INTERACTION_TYPES_BLUR)).toBe(true);
  });
});

// ── Parity with the Pill, and the chat face ─────────────────────────

/** A paste carrying one picture, as WebKit hands it over. */
function pasteImage(target: HTMLElement) {
  const file = new File(["x"], "shot.png", { type: "image/png" });
  fireEvent.paste(target, {
    clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => file }] },
  });
}

describe("IslandBar: pasted images", () => {
  const RealReader = globalThis.FileReader;
  beforeEach(() => {
    // Read at once: the picture's data URL is the file's name.
    class InstantReader {
      result: string | null = null;
      onload: (() => void) | null = null;
      readAsDataURL(file: File) {
        this.result = `data:image/png;base64,${file.name}`;
        this.onload?.();
      }
    }
    globalThis.FileReader = InstantReader as unknown as typeof FileReader;
  });
  afterEach(() => {
    globalThis.FileReader = RealReader;
  });

  it("a picture alone is a draft: the full box, a thumbnail, and Send sends it with the text", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    act(() => pasteImage(composer()));
    // Rust leaves its input state; the picture still holds the box open, full width.
    await send(UI.BAR_STATES_DEFAULT);
    act(() => composer().blur());
    expect(face()).toBe("compose");
    expect(screen.getByTestId("island-composer")).not.toHaveAttribute("data-compact");
    expect(screen.getByRole("img", { name: "Pasted image 1" })).toBeInTheDocument();
    const sendButton = screen.getByRole("button", { name: "Send" });
    expect(sendButton).toBeEnabled();
    fireEvent.click(sendButton);
    expect(invoke).toHaveBeenCalledWith("dispatch_query", {
      query: "",
      images: ["data:image/png;base64,shot.png"],
    });
    expect(screen.queryByTestId("island-thumbs")).toBeNull();
  });

  it("a thumbnail can be taken out again", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    act(() => pasteImage(composer()));
    act(() => pasteImage(composer()));
    expect(screen.getAllByRole("img", { name: /Pasted image/ })).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Remove image 1" }));
    expect(screen.getAllByRole("img", { name: /Pasted image/ })).toHaveLength(1);
  });
});

describe("IslandBar: wake phrase, dot, roster, tooltips", () => {
  it("the hover face carries the wake phrase toggle when a phrase is set, and it pauses the engine", async () => {
    backend.answers.get_triggers = [{ gesture: "say", enabled: true, phrase: "hey juno" }];
    backend.answers.get_always_listening_status = true;
    render(<IslandBar />);
    await act(async () => {});
    fireEvent.pointerEnter(stage());
    const toggle = screen.getByTestId("island-wake-toggle");
    expect(toggle).toHaveAccessibleName("Stop listening for the wake phrase");
    fireEvent.click(toggle);
    expect(invoke).toHaveBeenCalledWith("stop_always_listening_mode");
    // Rust says what happened; the toggle follows.
    await act(async () => {
      eventHandlers.get(EVENTS.VOICE_TRIGGER_LISTENING)?.({ listening: false, phrases: ["hey juno"] });
    });
    expect(screen.getByTestId("island-wake-toggle")).toHaveAccessibleName("Listen for the wake phrase");
  });

  it("no wake phrase, no toggle", async () => {
    render(<IslandBar />);
    await act(async () => {});
    fireEvent.pointerEnter(stage());
    expect(screen.queryByTestId("island-wake-toggle")).toBeNull();
  });

  it("the dot turns system red with Rust's reason when Juno cannot answer", async () => {
    backend.answers.get_connectivity = { status: "offline", label: "Network unavailable", provider: "Claude" };
    render(<IslandBar />);
    await act(async () => {});
    fireEvent.pointerEnter(stage());
    const dot = screen.getByTestId("island-dot-target");
    expect(dot).toHaveAccessibleName("Network unavailable");
    expect(dot).toHaveAttribute("data-tone", "down");
    // And back when Rust says so.
    await act(async () => {
      eventHandlers.get(EVENTS.SYSTEM_CONNECTIVITY_CHANGED)?.({
        status: "connected",
        label: "Connected to Claude",
        provider: "Claude",
      });
    });
    expect(screen.getByTestId("island-dot-target")).toHaveAccessibleName("Connected to Claude");
    expect(screen.getByTestId("island-dot-target")).not.toHaveAttribute("data-tone");
  });

  it("the roster appears beside the shape at two agents, and the shape does not move", async () => {
    const session = (id: string, focused = false) => ({
      id,
      agent_name: `Agent ${id}`,
      color_slot: 0,
      display_color: "#0A84FF",
      status: "running",
      current_action: null,
      started_at_ms: 0,
      last_activity_ms: 0,
      focused,
    });
    agents.sessions = [session("a", true)];
    const view = render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(screen.queryByTestId("island-roster")).toBeNull();
    const last = () => frameReport.mock.calls[frameReport.mock.calls.length - 1][1] as Array<Record<string, number>>;
    const before = last()[0];
    expect(last()).toHaveLength(1);

    agents.sessions = [session("a", true), session("b")];
    view.rerender(<IslandBar />);
    expect(screen.getByTestId("island-roster")).toBeInTheDocument();
    expect(screen.getByTestId("roster-dot-b")).toBeInTheDocument();
    // The shape's rect is unchanged; the roster is a second rect beside it.
    expect(last()[0]).toEqual(before);
    expect(last()).toHaveLength(2);
    expect(last()[1].x).toBeGreaterThanOrEqual(before.x + before.width);
    expect(last()[1].y).toBe(before.y);
    fireEvent.click(screen.getByTestId("roster-dot-b"));
    expect(agents.focusSession).toHaveBeenCalledWith("b");
  });

  it("opens a control's tooltip from the native forwarded hover, while another app is active", async () => {
    render(<IslandBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.SYSTEM_MOUSE_ENTERED_WINDOW)?.(null);
    });
    const mic = screen.getByRole("button", { name: "Talk to Juno" });
    const original = document.elementFromPoint;
    document.elementFromPoint = () => mic;
    try {
      await act(async () => {
        eventHandlers.get(EVENTS.SYSTEM_MOUSE_MOVED_WINDOW)?.({ x: 10, y: 10 });
      });
      expect(screen.queryByTestId("pill-tooltip")).toBeNull();
      await act(async () => {
        vi.advanceTimersByTime(PILL_TOOLTIP_DELAY_MS + 20);
      });
      expect(screen.getByTestId("pill-tooltip")).toHaveTextContent("Talk to Juno");
      // The title steps aside while the tooltip shows, so the two never double up.
      expect(mic).not.toHaveAttribute("title");
    } finally {
      document.elementFromPoint = original;
    }
  });
});

describe("IslandBar: chat face", () => {
  const HISTORY: ChatMessage[] = [
    { role: "user", content: "First question", timestamp: 1 },
    { role: "assistant", content: "First answer.", messageId: "h1", timestamp: 2 },
    ...ANSWER.map((m, i) => ({ ...m, timestamp: 3 + i })),
  ];

  it("grows out of the answer, shows the whole conversation, and keeps the one composer", async () => {
    renderWithAnswer(HISTORY);
    expect(face()).toBe("answer");
    const node = composer();
    fireEvent.click(screen.getByRole("button", { name: "Show the conversation" }));
    expect(face()).toBe("chat");
    expect(screen.getByTestId("chat-history")).toHaveTextContent("First question | First answer. | What is the weather | Sunny, 72.");
    expect(composer()).toBe(node);
    expect(composer()).toHaveAccessibleName("Follow up");
    // No ring in chat: it stays until put down.
    expect(screen.queryByTestId("island-linger")).toBeNull();
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(face()).toBe("chat");
    // The shape is the chat size, far bigger than the answer's cap.
    const shapeHit = (frameReport.mock.calls[frameReport.mock.calls.length - 1][1] as Array<{ width: number }>)[0];
    expect(shapeHit.width).toBeGreaterThan(360);
  });

  it("Escape steps down one rung at a time: chat, answer, capsule", async () => {
    renderWithAnswer();
    fireEvent.click(screen.getByRole("button", { name: "Show the conversation" }));
    expect(face()).toBe("chat");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("answer");
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("idle");
  });

  it("from a component, chat steps back to the component; its collapse control does the same", async () => {
    renderWithAnswer(COMPONENT_ANSWER);
    expect(face()).toBe("component");
    fireEvent.click(screen.getByRole("button", { name: "Show the conversation" }));
    expect(face()).toBe("chat");
    fireEvent.click(screen.getByRole("button", { name: "Back to the answer" }));
    expect(face()).toBe("component");
  });

  it("with no answer to go back to, chat steps down to the composer", async () => {
    const view = render(<IslandBar />);
    chat.messages = [{ role: "user", content: "Still thinking about it", timestamp: 1 }];
    view.rerender(<IslandBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.BAR_TOGGLE_PANE)?.(null);
    });
    expect(face()).toBe("chat");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(face()).toBe("compose");
    expect(document.activeElement).toBe(composer());
  });

  it("a new turn streams into the chat face rather than taking it away", async () => {
    const view = renderWithAnswer();
    fireEvent.click(screen.getByRole("button", { name: "Show the conversation" }));
    await send(UI.BAR_STATES_LISTENING);
    expect(face()).toBe("chat");
    await send(UI.BAR_STATES_AGENT_RESPONDING);
    chat.messages = [...ANSWER, { role: "user", content: "And tomorrow", timestamp: 5 }, { role: "assistant", content: "Rain.", messageId: "m2", timestamp: 6 }];
    view.rerender(<IslandBar />);
    expect(face()).toBe("chat");
    expect(screen.getByTestId("chat-history")).toHaveTextContent("Rain.");
  });
});
