import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { BAR_HEIGHT, BAR_WIDTH, DRAIN_MS, SHADOW_PAD, SHRINK_DELAY_MS } from "../narratorModel";

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
    theme: vi.fn(async () => "dark"),
    onThemeChanged: vi.fn(async () => () => {}),
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
vi.mock("@/hooks/useSystemTheme", () => ({
  useSystemTheme: () => "dark",
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

import { AppBar } from "../../app-bar";

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

const posture = () => screen.getByTestId("bar-root").getAttribute("data-posture");
const sheet = () => screen.getByTestId("bar-sheet").getAttribute("data-open");
const rail = () => screen.getByTestId("bar-rail").getAttribute("data-rail");
const REST_WINDOW = { width: BAR_WIDTH + 2 * SHADOW_PAD, height: BAR_HEIGHT + 2 * SHADOW_PAD };

const step = (description: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
  role: "tool_call_request",
  content: description,
  tool_name: "browser",
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

describe("AppBar", () => {
  it("rests as a wide strip that says Ask Juno, and sizes its window before it shows", async () => {
    render(<AppBar />);
    expect(posture()).toBe("rest");
    expect(screen.getByTestId("bar-empty")).toHaveTextContent("Ask Juno");
    // First paint: the window is sized at once, and the strip stays hidden until it is.
    expect(resizeWindowIfChanged).toHaveBeenCalledWith(REST_WINDOW);
    await act(async () => {});
    expect(screen.getByTestId("bar-root").style.opacity).toBe("1");
  });

  it("runs your words along the line as you speak, with the rail as your voice", async () => {
    render(<AppBar />);
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Do I need a coat", audioLevel: 0.5 });
    expect(posture()).toBe("listen");
    expect(screen.getByTestId("bar-question")).toHaveTextContent("Do I need a coat");
    expect(screen.getByTestId("bar-question").getAttribute("data-tone")).toBe("green");
    expect(rail()).toBe("meter");
    expect(screen.getByTestId("bar-dot").getAttribute("data-motion")).toBe("breathe");
  });

  it("never asks for a new window size while the line changes", async () => {
    render(<AppBar />);
    await act(async () => {});
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" });
    await send(UI.BAR_STATES_SUBMITTING, { lastSubmittedValue: "Send it" });
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Send it" });
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });

  it("narrates each step as a bead, and only the newest keeps its words", async () => {
    const { rerender } = render(<AppBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Q" });
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      step("open weather.com", { success: true }),
      step("read the forecast"),
    ];
    rerender(<AppBar />);
    expect(screen.getByTestId("bar-question")).toHaveTextContent("Q");
    expect(screen.getAllByTestId("bar-bead")).toHaveLength(2);
    expect(screen.getAllByTestId("bar-step-collapsed")).toHaveLength(1);
    expect(screen.getByTestId("bar-step")).toHaveTextContent("read the forecast");
    expect(screen.getByTestId("bar-step").getAttribute("data-tone")).toBe("live");
    expect(rail()).toBe("toBead");
  });

  it("stops the line at a step waiting on you, and tells the backend what you decided", async () => {
    const { rerender } = render(<AppBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Send it" });
    chat.messages = [
      { role: "user", content: "Send it", timestamp: 1 },
      step("send the email", { tool_id: "t1", approval_state: "pending" }),
    ];
    rerender(<AppBar />);
    expect(screen.getByTestId("bar-approval")).toHaveTextContent("Juno wants to send the email");
    expect(sheet()).toBe("false");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("ends the line with the answer's first sentence, and keeps a one-line answer on the strip", async () => {
    const { rerender } = render(<AppBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Send it" });
    chat.messages = [
      { role: "user", content: "Send it", timestamp: 1 },
      { role: "assistant", content: "Done.", messageId: "m1", isStreaming: true, timestamp: 2 },
    ];
    rerender(<AppBar />);
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Done.");
    expect(rail()).toBe("streaming");
    expect(sheet()).toBe("false");
    // The window did not grow for a one-line answer.
    expect(resizeWindowIfChanged).toHaveBeenCalledTimes(1);
  });

  it("slides the sheet down for a longer answer, growing the window first", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Weather" });
    chat.messages = [
      { role: "user", content: "Weather", timestamp: 1 },
      {
        role: "assistant",
        content: "Yes, bring a coat. It is 41 degrees and windy.",
        messageId: "m1",
        isStreaming: true,
        timestamp: 2,
      },
    ];
    rerender(<AppBar />);
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Yes, bring a coat.");
    // Growing: the window was resized before the sheet was shown.
    expect(resizeWindowIfChanged).toHaveBeenCalledTimes(1);
    const asked = resizeWindowIfChanged.mock.calls[0][0];
    expect(asked.width).toBe(REST_WINDOW.width);
    expect(asked.height).toBeGreaterThan(REST_WINDOW.height);
    await act(async () => {});
    expect(sheet()).toBe("true");
    expect(screen.getByTestId("answer")).toHaveTextContent("It is 41 degrees and windy.");
  });

  it("drains once the answer is done, then closes the sheet and shrinks the window after the slide", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Weather" });
    chat.messages = [
      { role: "user", content: "Weather", timestamp: 1 },
      { role: "assistant", content: "Yes. Bring a coat.", messageId: "m1", isStreaming: true, timestamp: 2 },
    ];
    rerender(<AppBar />);
    await act(async () => {});
    expect(sheet()).toBe("true");
    // Still streaming: no drain.
    expect(rail()).toBe("streaming");
    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<AppBar />);
    await send(UI.BAR_STATES_DEFAULT);
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(rail()).toBe("drain");
    resizeWindowIfChanged.mockClear();
    await act(async () => {
      vi.advanceTimersByTime(DRAIN_MS + 100);
    });
    expect(sheet()).toBe("false");
    // The turn stays on the line, dimmed, and the rail is empty.
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Yes.");
    expect(screen.getByTestId("bar-question").getAttribute("data-tone")).toBe("dim");
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenCalledWith(REST_WINDOW);
  });

  it("holds the drain while the pointer is over the strip, and Escape closes the sheet at once", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "One. Two.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<AppBar />);
    await act(async () => {});
    expect(sheet()).toBe("true");
    fireEvent.pointerEnter(screen.getByTestId("bar-root"));
    await act(async () => {
      vi.advanceTimersByTime(DRAIN_MS * 2);
    });
    expect(sheet()).toBe("true");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(sheet()).toBe("false");
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("does not open the sheet for an answer that was already there when it mounted", () => {
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Old. Answer.", messageId: "old", timestamp: 2 },
    ];
    render(<AppBar />);
    expect(sheet()).toBe("false");
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Old.");
  });

  it("closes the sheet and clears the line when you speak again", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "One. Two.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<AppBar />);
    await act(async () => {});
    expect(sheet()).toBe("true");
    await send(UI.BAR_STATES_LISTENING);
    expect(sheet()).toBe("false");
    expect(posture()).toBe("listen");
    expect(screen.queryByTestId("bar-answer")).toBeNull();
  });

  it("turns the rail red at the step that failed, with the message", async () => {
    const { rerender } = render(<AppBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      step("open weather.com", { success: true }),
      step("read the forecast", { success: false }),
    ];
    rerender(<AppBar />);
    await send(UI.BAR_STATES_ERROR, { lastSubmittedValue: "Q", currentError: "The page timed out" });
    expect(rail()).toBe("failed");
    expect(screen.getByTestId("bar-step")).toHaveTextContent("read the forecast: The page timed out");
    expect(screen.getByTestId("bar-step").getAttribute("data-tone")).toBe("error");
    const beads = screen.getAllByTestId("bar-bead");
    expect(beads[beads.length - 1].getAttribute("data-failed")).toBe("true");
    expect(screen.getByTestId("bar-dot").getAttribute("data-motion")).toBe("shake");
  });

  it("keeps the spoken channel behind one button in the sheet", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      {
        role: "assistant",
        content: "Here is the list. Two items.",
        messageId: "m1",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Here you go."], total_spoken_text: "Here you go." },
      },
    ];
    rerender(<AppBar />);
    await act(async () => {});
    expect(screen.queryByTestId("bar-spoken")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Spoken aloud" }));
    expect(screen.getByTestId("bar-spoken")).toHaveTextContent("Here you go.");
  });

  it("shows a spoken-only reply as the line itself", () => {
    const { rerender } = render(<AppBar />);
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
    rerender(<AppBar />);
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Done.");
    expect(sheet()).toBe("false");
  });

  it("clicking the answer line opens the sheet by hand", async () => {
    const { rerender } = render(<AppBar />);
    await act(async () => {});
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Done.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<AppBar />);
    expect(sheet()).toBe("false");
    fireEvent.click(screen.getByRole("button", { name: "Show the full answer" }));
    await act(async () => {});
    expect(sheet()).toBe("true");
  });

  it("Escape while Juno works stops the work instead of closing", async () => {
    render(<AppBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_ESCAPE }),
      }),
    );
  });

  it("clicking the resting strip asks Rust to open the composer", async () => {
    render(<AppBar />);
    await act(async () => {});
    fireEvent.click(screen.getByTestId("bar-strip"));
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        elementId: "app-bar",
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_CLICK }),
      }),
    );
  });

  it("types in the question slot and submits on return, keeping the last turn beside it", async () => {
    const { rerender } = render(<AppBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Done.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<AppBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("compose");
    const input = screen.getByRole("textbox", { name: "Ask Juno" });
    expect(input).toHaveAttribute("placeholder", "Follow up");
    expect(screen.getByTestId("bar-answer")).toHaveTextContent("Done.");
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

  it("narrates Juno driving the cursor as the live bead", async () => {
    render(<AppBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Click it" });
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, tool: "computer", target_app: "Safari" });
    });
    expect(screen.getByTestId("bar-driving")).toHaveTextContent("using the mouse in Safari");
    expect(screen.getByTestId("bar-dot").getAttribute("data-motion")).toBe("orbit");
  });
});
