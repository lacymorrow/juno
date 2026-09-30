import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { LINGER_MS, SHRINK_DELAY_MS } from "../studioModel";

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

import { VoiceAIBar } from "../../voice-ai-bar";

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

const posture = () => screen.getByTestId("studio-shell").getAttribute("data-posture");
const wave = () => screen.getByTestId("studio-wave");
const root = () => screen.getByTestId("studio-shell").closest(".h-screen") as HTMLElement;

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  chat.handleApprovalUpdate.mockClear();
  chat.messages = [];
  chat.isProcessing = false;
});

afterEach(() => {
  vi.useRealTimers();
});

describe("VoiceAIBar (Studio)", () => {
  it("rests as a deck with a flat hairline, and opens a take that follows your voice", async () => {
    render(<VoiceAIBar />);
    expect(posture()).toBe("deck");
    expect(wave().getAttribute("data-mode")).toBe("flat");
    expect(wave().getAttribute("data-live")).toBe("false");
    await send(UI.BAR_STATES_LISTENING, { audioLevel: 0.8 });
    expect(posture()).toBe("take");
    expect(wave().getAttribute("data-mode")).toBe("live");
    expect(wave().getAttribute("data-color")).toBe("#30D158");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Listening");
    // The strip samples the level and comes alive.
    await act(async () => {
      vi.advanceTimersByTime(250);
    });
    expect(wave().getAttribute("data-live")).toBe("true");
  });

  it("rolls your words up the teleprompter, provisional words lighter, final words solid", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_TRANSCRIBING, { transcriptionText: "Send the draft", transcriptionProvisional: false });
    expect(posture()).toBe("take");
    expect(screen.getByTestId("studio-prompter")).toHaveTextContent("Send the draft");
    expect(screen.queryByTestId("studio-provisional")).toBeNull();
    await send(UI.BAR_STATES_TRANSCRIBING, {
      transcriptionText: "Send the draft to May",
      transcriptionProvisional: true,
    });
    expect(screen.getByTestId("studio-provisional")).toHaveTextContent("to May");
  });

  it("keeps what you asked in view while Juno works, starts the tape, and does not lurch", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" });
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_SUBMITTING, { lastSubmittedValue: "Send it" });
    expect(posture()).toBe("work");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Send it");
    expect(screen.getByTestId("studio-counter").getAttribute("data-mode")).toBe("running");
    expect(screen.getByTestId("studio-counter")).toHaveTextContent("00:00.0");
    await act(async () => {
      vi.advanceTimersByTime(1250);
    });
    expect(screen.getByTestId("studio-counter")).toHaveTextContent("00:01.2");
    // Take and work share a window size: no resize was asked for.
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });

  it("holds the counter when the work stops and forgets it at the next take", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    await send(UI.BAR_STATES_SUCCESS);
    expect(screen.getByTestId("studio-counter").getAttribute("data-mode")).toBe("held");
    const held = screen.getByTestId("studio-counter").textContent;
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    expect(screen.getByTestId("studio-counter").textContent).toBe(held);
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Done");
    await send(UI.BAR_STATES_LISTENING);
    expect(screen.queryByTestId("studio-counter")).toBeNull();
  });

  it("grows the window before it grows, and shrinks it after the spring", async () => {
    render(<VoiceAIBar />);
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith({ width: 180, height: 78 });
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING);
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ width: 368, height: 114 });
    expect(posture()).toBe("take");
  });

  it("speaks in blue from the sentence being spoken, with no mic level at all", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_SPEAKING, { spokenText: "Here you go.", audioLevel: 0 });
    expect(posture()).toBe("work");
    expect(wave().getAttribute("data-mode")).toBe("speech");
    expect(wave().getAttribute("data-color")).toBe("#0A84FF");
    expect(screen.getByTestId("studio-line").getAttribute("data-tone")).toBe("speech");
    await act(async () => {
      vi.advanceTimersByTime(300);
    });
    expect(wave().getAttribute("data-live")).toBe("true");
  });

  it("shows an open mic as a drifting blue baseline, never red", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_ALWAYS_LISTENING);
    expect(posture()).toBe("take");
    expect(wave().getAttribute("data-mode")).toBe("drift");
    expect(wave().getAttribute("data-color")).toBe("#0A84FF");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Mic open");
  });

  it("fails in red, and only then", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_ERROR, { currentError: "No network" });
    expect(wave().getAttribute("data-color")).toBe("#FF453A");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("No network");
    expect(screen.getByTestId("studio-line").getAttribute("data-tone")).toBe("error");
  });

  it("opens the script when an answer arrives, as You and Juno lines, then settles once the tape runs out", async () => {
    const { rerender } = render(<VoiceAIBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "What is the weather" });
    chat.messages = [
      { role: "user", content: "What is the weather", timestamp: 1 },
      { role: "assistant", content: "Sunny, 72.", messageId: "m1", isStreaming: true, timestamp: 2 },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("script");
    expect(screen.getByTestId("studio-you")).toHaveTextContent("What is the weather");
    expect(screen.getByTestId("studio-juno")).toHaveTextContent("Sunny, 72.");
    // Still streaming: the tape does not run.
    expect(screen.getByTestId("studio-tape").getAttribute("data-counting")).toBe("false");
    expect(screen.getByTestId("studio-counter").getAttribute("data-mode")).toBe("running");
    await act(async () => {
      vi.advanceTimersByTime(700);
    });

    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<VoiceAIBar />);
    await send(UI.BAR_STATES_DEFAULT);
    expect(screen.getByTestId("studio-tape").getAttribute("data-counting")).toBe("true");
    expect(screen.getByTestId("studio-counter").getAttribute("data-mode")).toBe("held");
    expect(screen.getByTestId("studio-counter")).toHaveTextContent("00:00.7");

    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(posture()).toBe("deck");
  });

  it("holds the tape while the pointer is over the studio, and Escape closes at once", async () => {
    const { rerender } = render(<VoiceAIBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("script");
    expect(screen.getByTestId("studio-tape").getAttribute("data-counting")).toBe("true");

    fireEvent.pointerEnter(root());
    expect(screen.getByTestId("studio-tape").getAttribute("data-paused")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(posture()).toBe("script");

    fireEvent.keyDown(document, { key: "Escape" });
    expect(posture()).toBe("deck");
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("settles to the deck, not the type, when a script is dismissed under focus", async () => {
    const { rerender } = render(<VoiceAIBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<VoiceAIBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("script");
    expect(screen.getByRole("textbox", { name: "Follow up" })).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_BLUR }),
      }),
    );
  });

  it("does not open the script for an answer that was already there when it mounted", () => {
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Old", messageId: "old", timestamp: 2 },
    ];
    render(<VoiceAIBar />);
    expect(posture()).toBe("deck");
  });

  it("closes the script when you speak again", async () => {
    const { rerender } = render(<VoiceAIBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("script");
    await send(UI.BAR_STATES_LISTENING);
    expect(posture()).toBe("take");
  });

  it("reads a running tool as a stage direction, in work and in the script", async () => {
    const { rerender } = render(<VoiceAIBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Open it" });
    chat.messages = [
      { role: "user", content: "Open it", timestamp: 1 },
      { role: "tool_call_request", content: "open the page in Safari", tool_name: "browser", tool_id: "t1" },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("work");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Juno runs open the page in Safari");
    expect(screen.getByTestId("studio-line").getAttribute("data-tone")).toBe("direction");

    chat.messages = [
      ...chat.messages.map((m) => (m.role === "tool_call_request" ? { ...m, success: true } : m)),
      { role: "assistant", content: "Opened.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("script");
    const direction = screen.getByTestId("studio-direction");
    expect(direction).toHaveTextContent("Juno ran open the page in Safari");
    expect(direction.getAttribute("data-kind")).toBe("done");
  });

  it("shows a tool waiting on you as a stage direction, and tells the backend what you decided", async () => {
    const { rerender } = render(<VoiceAIBar />);
    chat.messages = [
      { role: "user", content: "Send it", timestamp: 1 },
      {
        role: "tool_call_request",
        content: "send the email to Maya",
        tool_name: "mail",
        tool_id: "t1",
        approval_state: "pending",
      },
    ];
    rerender(<VoiceAIBar />);
    expect(posture()).toBe("script");
    expect(screen.getByTestId("studio-approval")).toHaveTextContent("Juno asks to send the email to Maya");
    expect(screen.getByTestId("studio-tape").getAttribute("data-counting")).toBe("false");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("puts what was said aloud on the Juno line and what was shown in the notes, once each", () => {
    const { rerender } = render(<VoiceAIBar />);
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
    rerender(<VoiceAIBar />);
    expect(screen.getByTestId("studio-spoken")).toHaveTextContent("Here you go.");
    expect(screen.getByTestId("studio-notes")).toHaveTextContent("Here is the list.");
    expect(screen.getAllByText("Here you go.")).toHaveLength(1);
  });

  it("a spoken-only reply is the Juno line itself, with no notes", () => {
    const { rerender } = render(<VoiceAIBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Done."], total_spoken_text: "Done." },
      },
    ];
    rerender(<VoiceAIBar />);
    expect(screen.getByTestId("studio-spoken")).toHaveTextContent("Done.");
    expect(screen.queryByTestId("studio-notes")).toBeNull();
  });

  it("Escape while Juno works stops the work instead of closing", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_ESCAPE }),
      }),
    );
  });

  it("types in the type posture and submits on return", async () => {
    render(<VoiceAIBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("type");
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

  it("says when Juno is driving the cursor, as a stage direction", async () => {
    render(<VoiceAIBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, tool: "click", target_app: "Mail" });
    });
    expect(posture()).toBe("work");
    expect(screen.getByTestId("studio-line")).toHaveTextContent("Juno is using the mouse in Mail");
    expect(wave().getAttribute("data-mode")).toBe("flat");
  });
});
