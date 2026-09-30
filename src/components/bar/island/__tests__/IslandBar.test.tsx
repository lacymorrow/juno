import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { LINGER_MS, SHRINK_DELAY_MS } from "../islandModel";

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

const posture = () => screen.getByTestId("island-shell").getAttribute("data-posture");

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

describe("IslandBar", () => {
  it("rests as a capsule and opens an ear with your words as you speak", async () => {
    render(<IslandBar />);
    expect(posture()).toBe("capsule");
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft to Maya" });
    expect(posture()).toBe("ear");
    expect(screen.getByTestId("island-words")).toHaveTextContent("Send the draft to Maya");
    expect(screen.getByTestId("island-dot").getAttribute("data-motion")).toBe("breathe");
  });

  it("keeps what you asked in view while Juno works, and does not lurch wider", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" });
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_SUBMITTING, { lastSubmittedValue: "Send it" });
    expect(posture()).toBe("status");
    expect(screen.getByTestId("island-words")).toHaveTextContent("Send it");
    // Ear and status share a window size: no resize was asked for.
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
  });

  it("grows the window before it grows, and shrinks it after the spring", async () => {
    render(<IslandBar />);
    // Mount: the window shrinks to the capsule after the spring.
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith({ width: 140, height: 76 });
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING);
    // Growing: the resize came first, at once.
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ width: 348, height: 80 });
    expect(posture()).toBe("ear");
  });

  it("opens the card when an answer arrives, then settles back once the ring runs out", async () => {
    const { rerender } = render(<IslandBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "What is the weather" });
    chat.messages = [
      { role: "user", content: "What is the weather", timestamp: 1 },
      { role: "assistant", content: "Sunny, 72.", messageId: "m1", isStreaming: true, timestamp: 2 },
    ];
    rerender(<IslandBar />);
    expect(posture()).toBe("card");
    expect(screen.getByTestId("answer")).toHaveTextContent("Sunny, 72.");
    // Still streaming: the ring does not count.
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");

    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<IslandBar />);
    await send(UI.BAR_STATES_DEFAULT);
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");

    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 100);
    });
    expect(posture()).toBe("capsule");
  });

  it("holds the ring while the pointer is over the island, and Escape closes at once", async () => {
    const { rerender } = render(<IslandBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<IslandBar />);
    expect(posture()).toBe("card");
    const ring = screen.getByTestId("island-linger");
    expect(ring.getAttribute("data-counting")).toBe("true");

    fireEvent.pointerEnter(ring.closest(".h-screen") as HTMLElement);
    expect(screen.getByTestId("island-linger").getAttribute("data-paused")).toBe("true");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(posture()).toBe("card");

    fireEvent.keyDown(document, { key: "Escape" });
    expect(posture()).toBe("capsule");
    // Idle and nothing working: Escape closed locally, it did not stop anything.
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("settles to the capsule, not the line, when a card is dismissed under focus", async () => {
    const { rerender } = render(<IslandBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<IslandBar />);
    // Clicking the card focused the window, so Rust moved to its input state.
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("card");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_BLUR }),
      }),
    );
  });

  it("does not open the card for an answer that was already there when it mounted", () => {
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Old", messageId: "old", timestamp: 2 },
    ];
    render(<IslandBar />);
    expect(posture()).toBe("capsule");
  });

  it("closes the card when you speak again", async () => {
    const { rerender } = render(<IslandBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<IslandBar />);
    expect(posture()).toBe("card");
    await send(UI.BAR_STATES_LISTENING);
    expect(posture()).toBe("ear");
  });

  it("shows a tool waiting on you, and tells the backend what you decided", async () => {
    const { rerender } = render(<IslandBar />);
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
    rerender(<IslandBar />);
    expect(posture()).toBe("card");
    expect(screen.getByTestId("island-approval")).toHaveTextContent("Juno wants to open the page in Safari");
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("keeps the spoken channel behind one button", async () => {
    const { rerender } = render(<IslandBar />);
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
    rerender(<IslandBar />);
    expect(screen.queryByTestId("island-spoken")).toBeNull();
    const root = screen.getByTestId("island-shell").closest(".h-screen") as HTMLElement;
    fireEvent.pointerEnter(root);
    fireEvent.click(screen.getByRole("button", { name: "Spoken aloud" }));
    expect(screen.getByTestId("island-spoken")).toHaveTextContent("Here you go.");
    // Pressing a button cancels the ring outright while the pointer stays.
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("false");
    // Once the pointer leaves, a fresh ring starts.
    fireEvent.pointerLeave(root);
    expect(screen.getByTestId("island-linger").getAttribute("data-counting")).toBe("true");
    expect(screen.getByTestId("island-linger").getAttribute("data-paused")).toBe("false");
  });

  it("shows a spoken-only reply as the body itself", () => {
    const { rerender } = render(<IslandBar />);
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
    rerender(<IslandBar />);
    expect(screen.getByTestId("island-spoken-only")).toHaveTextContent("Done.");
    expect(screen.queryByRole("button", { name: "Spoken aloud" })).toBeNull();
  });

  it("Escape while Juno works stops the work instead of closing", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      expect.objectContaining({
        interaction: expect.objectContaining({ interaction_type: UI.INTERACTION_TYPES_ESCAPE }),
      }),
    );
  });

  it("types in the line and submits on return", async () => {
    render(<IslandBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("line");
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
});
