import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { HOVER_CLOSE_MS, HOVER_OPEN_MS, LINGER_MS, SHRINK_DELAY_MS, STAGE } from "../orbModel";

// ── Tauri, hook and Three.js stand-ins ──────────────────────────────

const { invoke, listenHandlers, eventHandlers, resizeWindowIfChanged, chat, canvas } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(true)),
  listenHandlers: new Map<string, (event: { payload: unknown }) => void>(),
  eventHandlers: new Map<string, (payload: unknown) => void>(),
  resizeWindowIfChanged: vi.fn((_size: { width: number; height: number }) => Promise.resolve()),
  chat: {
    messages: [] as ChatMessage[],
    isProcessing: false,
    handleApprovalUpdate: vi.fn(),
  },
  canvas: { onSettled: null as null | (() => void) },
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
vi.mock("@/components/input-control/InputControlNotices", () => ({
  InputControlNotices: () => null,
}));
vi.mock("@/components/ui/mixed-content-renderer", () => ({
  MixedContentRenderer: ({ content }: { content: string }) => <div data-testid="answer">{content}</div>,
}));
// Three.js never loads in tests: the canvas is a div that reports what the
// bar asked of it, and lets a test say the orb has settled.
vi.mock("../OrbCanvas", () => ({
  OrbCanvas: ({
    drive,
    frameloop,
    onSettled,
  }: {
    drive: { current: { look: { motion: string; scale: number }; targets: { scale: number } } };
    frameloop: string;
    onSettled: () => void;
  }) => {
    canvas.onSettled = onSettled;
    return (
      <div
        data-testid="orb-canvas"
        data-frameloop={frameloop}
        data-motion={drive.current.look.motion}
        data-scale={String(drive.current.targets.scale)}
      />
    );
  },
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

import { ElevenLabsOrbBar } from "../../elevenlabs-orb-bar";

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

/** Let pending window resizes land, so a grown posture can show. A sheet
 *  opening under a caption grows twice, so this settles more than once. */
const flush = async () => {
  for (let i = 0; i < 3; i += 1) await act(async () => {});
};

const bar = () => screen.getByTestId("orb-bar");
const posture = () => bar().getAttribute("data-posture");
const loop = () => bar().getAttribute("data-loop");
const motion = () => screen.getByTestId("orb").getAttribute("data-motion");
const tint = () => screen.getByTestId("orb").getAttribute("data-tint");

const interaction = (type: string, data?: Record<string, unknown>) =>
  expect.objectContaining({
    interaction: expect.objectContaining(data ? { interaction_type: type, data } : { interaction_type: type }),
  });

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  invoke.mockClear();
  resizeWindowIfChanged.mockClear();
  chat.messages = [];
  chat.isProcessing = false;
  chat.handleApprovalUpdate.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("ElevenLabsOrbBar", () => {
  it("rests as the orb alone, in a window the size of its stage, and lets the loop sleep once settled", async () => {
    render(<ElevenLabsOrbBar />);
    expect(posture()).toBe("orb");
    expect(motion()).toBe("breathe");
    expect(loop()).toBe("always");
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith({ width: STAGE, height: STAGE });
    await act(async () => {
      canvas.onSettled?.();
    });
    expect(loop()).toBe("demand");
    // Anything Rust says wakes it.
    await send(UI.BAR_STATES_DEFAULT);
    expect(loop()).toBe("always");
  });

  it("swells blue with your words under it, growing the window before the caption shows", async () => {
    render(<ElevenLabsOrbBar />);
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft to Maya", audioLevel: 0.8 });
    expect(motion()).toBe("swell");
    expect(tint()).toBe("#0A84FF");
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ width: 360, height: 170 });
    expect(posture()).toBe("caption");
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("Send the draft to Maya");
    expect(screen.getByTestId("orb-caption").getAttribute("data-tone")).toBe("live");
    // The audio level reached the canvas as a bigger orb.
    expect(Number(screen.getByTestId("orb-canvas").getAttribute("data-scale"))).toBeGreaterThan(0.78);
  });

  it("goes green while your words become text", async () => {
    render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_DICTATING, { transcriptionText: "Hello", transcriptionProvisional: true });
    expect(tint()).toBe("#30D158");
    expect(screen.getByTestId("orb-caption").getAttribute("data-tone")).toBe("provisional");
  });

  it("hides the caption at once and shrinks the window only after it has faded", async () => {
    render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" });
    expect(posture()).toBe("caption");
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_DEFAULT);
    expect(posture()).toBe("orb");
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenCalledWith({ width: STAGE, height: STAGE });
  });

  it("keeps your question under the turning orb, and names the tool that runs", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Send it" });
    expect(motion()).toBe("spin");
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("Send it");
    expect(screen.getByTestId("orb-caption").getAttribute("data-tone")).toBe("dim");
    chat.messages = [
      { role: "user", content: "Send it", timestamp: 1 },
      { role: "tool_call_request", content: "Sending the draft", tool_name: "mail" },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("Sending the draft");
  });

  it("captions the answer one spoken sentence at a time, lingers, then settles", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_AGENT_RESPONDING, { lastSubmittedValue: "Send it" });
    chat.messages = [
      { role: "user", content: "Send it", timestamp: 1 },
      {
        role: "assistant",
        content: "Done. The draft is with Maya.",
        messageId: "m1",
        isStreaming: true,
        timestamp: 2,
        tts_metadata: {
          has_spoken_content: true,
          tts_parts: ["Done.", "The draft is with Maya."],
          total_spoken_text: "Done. The draft is with Maya.",
        },
      },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("caption");
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("The draft is with Maya.");
    // No component: the sheet stays folded unless asked for.
    expect(screen.queryByTestId("orb-sheet")).toBeNull();

    chat.messages = [chat.messages[0], { ...chat.messages[1], isStreaming: false }];
    rerender(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_DEFAULT);
    expect(posture()).toBe("caption");
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS + 200);
    });
    expect(posture()).toBe("orb");
  });

  it("unfolds the sheet on hover for the whole answer, and folds it when the pointer leaves", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A long answer.", messageId: "m1", timestamp: 2 },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("caption");
    fireEvent.pointerEnter(bar());
    await act(async () => {
      vi.advanceTimersByTime(HOVER_OPEN_MS + 10);
    });
    await flush();
    expect(posture()).toBe("sheet");
    expect(screen.getByTestId("answer")).toHaveTextContent("A long answer.");
    // The ring does not run while the pointer is over the orb.
    await act(async () => {
      vi.advanceTimersByTime(LINGER_MS * 2);
    });
    expect(posture()).toBe("sheet");
    fireEvent.pointerLeave(bar());
    await act(async () => {
      vi.advanceTimersByTime(HOVER_CLOSE_MS + 10);
    });
    expect(posture()).toBe("caption");
  });

  it("opens the sheet by itself when the answer carries a component", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    chat.messages = [
      { role: "user", content: "Weather", timestamp: 1 },
      { role: "assistant", content: 'Sunny. <WeatherCard city="x" />', messageId: "m1", isJsx: true, timestamp: 2 },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("sheet");
    expect(screen.getByTestId("answer")).toHaveTextContent("Sunny.");
    // The sheet is the whole answer; the line is not shown twice.
    expect(screen.queryByTestId("orb-caption")).toBeNull();
  });

  it("does not linger for an answer that was already there when it mounted", () => {
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Old", messageId: "old", timestamp: 2 },
    ];
    render(<ElevenLabsOrbBar />);
    expect(posture()).toBe("orb");
  });

  it("lets go of a lingering answer when you speak again", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("caption");
    await send(UI.BAR_STATES_LISTENING);
    expect(posture()).toBe("orb");
  });

  it("holds still and makes the caption the question when a tool waits on you", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Open it" });
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
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("approval");
    expect(motion()).toBe("still");
    expect(screen.getByTestId("orb-approval")).toHaveTextContent("Juno wants to open the page in Safari");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    });
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
    expect(chat.handleApprovalUpdate).toHaveBeenCalledWith("t1", "approved");
  });

  it("flinches red and says what went wrong", async () => {
    render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_ERROR, { currentError: "Connection unavailable" });
    expect(motion()).toBe("flinch");
    expect(tint()).toBe("#FF453A");
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("Connection unavailable");
    expect(screen.getByTestId("orb-caption").getAttribute("data-tone")).toBe("error");
  });

  it("Escape stops work while Juno works, and settles a lingering answer otherwise", async () => {
    const { rerender } = render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_LOADING, { lastSubmittedValue: "Go" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction(UI.INTERACTION_TYPES_ESCAPE));
    invoke.mockClear();
    await send(UI.BAR_STATES_DEFAULT);
    chat.messages = [
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A", messageId: "m1", timestamp: 2 },
    ];
    rerender(<ElevenLabsOrbBar />);
    await flush();
    expect(posture()).toBe("caption");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(posture()).toBe("orb");
    expect(invoke).not.toHaveBeenCalledWith("ui_handle_interaction", expect.anything());
  });

  it("becomes a composer when Rust is in its input state, and submits on return", async () => {
    render(<ElevenLabsOrbBar />);
    await send(UI.BAR_STATES_INPUT);
    expect(posture()).toBe("caption");
    const input = screen.getByRole("textbox", { name: "Ask Juno" });
    fireEvent.change(input, { target: { value: "Send it" } });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value: "Send it" }),
    );
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction(UI.INTERACTION_TYPES_SUBMIT, { value: "Send it" }),
    );
  });

  it("a click on the resting orb asks Rust to open", async () => {
    render(<ElevenLabsOrbBar />);
    fireEvent.click(screen.getByTestId("orb"));
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction(UI.INTERACTION_TYPES_CLICK));
  });

  it("says Juno is using the mouse while it drives", async () => {
    render(<ElevenLabsOrbBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, tool: "computer", target_app: "Safari" });
    });
    expect(motion()).toBe("spin");
    expect(screen.getByTestId("orb-caption")).toHaveTextContent("Juno is using the mouse in Safari");
  });
});
