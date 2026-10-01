import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import { HELD_MS, SHRINK_DELAY_MS, STAGE } from "../shaderOrbModel";

// ── Tauri, hook and WebGL stand-ins ──────────────────────────────

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
// jsdom has no WebGL: the canvas is a div that reports what the bar asked of
// it, and lets a test say the loop has gone to sleep.
vi.mock("../OrbShaderCanvas", () => ({
  OrbShaderCanvas: ({
    drive,
    frameloop,
    onSettled,
    size,
  }: {
    drive: { current: { look: { motion: string; hue: number; mono: number }; targets: { scale: number; ripple: number } } };
    frameloop: string;
    onSettled: () => void;
    size: number;
  }) => {
    canvas.onSettled = onSettled;
    return (
      <div
        data-testid="orb-shader"
        data-frameloop={frameloop}
        data-size={String(size)}
        data-scale={String(drive.current.targets.scale)}
        data-ripple={String(drive.current.targets.ripple)}
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

import { ShaderOrbBar } from "../../shader-orb-bar";

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

/** Let pending window resizes land, so a grown posture can show. */
const flush = async () => {
  for (let i = 0; i < 3; i += 1) await act(async () => {});
};

const bar = () => screen.getByTestId("orb-bar");
const posture = () => bar().getAttribute("data-posture");
const loop = () => bar().getAttribute("data-loop");
const orb = () => screen.getByTestId("orb");
const motion = () => orb().getAttribute("data-motion");
const hue = () => Number(orb().getAttribute("data-hue"));
const mono = () => Number(orb().getAttribute("data-mono"));
const scale = () => Number(screen.getByTestId("orb-shader").getAttribute("data-scale"));
const ripple = () => Number(screen.getByTestId("orb-shader").getAttribute("data-ripple"));

const interaction = (type: string) =>
  expect.objectContaining({
    interaction: expect.objectContaining({ interaction_type: type }),
  });

const assistant = (over: Partial<ChatMessage> = {}): ChatMessage =>
  ({ role: "assistant", content: "It is 3pm.", timestamp: 2, messageId: "a1", ...over }) as ChatMessage;
const user = (content: string): ChatMessage =>
  ({ role: "user", content, timestamp: 1 }) as ChatMessage;

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

describe("ShaderOrbBar", () => {
  it("rests as the sphere alone, in a window the size of its stage, and lets the loop sleep", async () => {
    render(<ShaderOrbBar />);
    expect(posture()).toBe("orb");
    expect(motion()).toBe("ember");
    expect(loop()).toBe("always");
    expect(screen.getByTestId("orb-shader").getAttribute("data-size")).toBe(String(STAGE));
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

  it("says nothing while you speak: it cools, swells and ripples instead", async () => {
    render(<ShaderOrbBar />);
    await send(UI.BAR_STATES_LISTENING, {
      transcriptionText: "Send the draft to Maya",
      audioLevel: 0.9,
    });
    await flush();
    expect(motion()).toBe("hear");
    expect(hue()).toBe(350);
    expect(posture()).toBe("orb");
    expect(screen.queryByTestId("orb-slot")).toBeNull();
    const loudScale = scale();
    const loudRipple = ripple();
    await send(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft to Maya", audioLevel: 0 });
    expect(scale()).toBeLessThan(loudScale);
    expect(ripple()).toBeLessThan(loudRipple);
  });

  it("says nothing while Juno works: it turns, and quickens as steps finish", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("tidy my desktop")];
    await send(UI.BAR_STATES_LOADING);
    await flush();
    expect(motion()).toBe("turn");
    expect(posture()).toBe("orb");
    const before = Number(orb().getAttribute("data-hue"));
    chat.messages = [
      user("tidy my desktop"),
      { role: "tool_call_request", content: "list the desktop", timestamp: 2, success: true } as ChatMessage,
    ];
    await send(UI.BAR_STATES_LOADING);
    // Still its own colours: thinking is motion, not a repaint.
    expect(Number(orb().getAttribute("data-hue"))).toBe(before);
    expect(motion()).toBe("turn");
  });

  it("stops dead and asks, when a tool is waiting on you", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [
      user("delete the old builds"),
      {
        role: "tool_call_request",
        content: "delete 12 files",
        timestamp: 2,
        approval_state: "pending",
        tool_id: "t1",
      } as ChatMessage,
    ];
    await send(UI.BAR_STATES_LOADING);
    await flush();
    expect(motion()).toBe("hold");
    expect(posture()).toBe("approval");
    expect(screen.getByTestId("orb-approval").textContent).toContain("Juno wants to delete 12 files");
    fireEvent.click(screen.getByText("Allow"));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("approve_tool_execution", { toolId: "t1" });
  });

  it("goes red and holds, with the error itself under it", async () => {
    render(<ShaderOrbBar />);
    await send(UI.BAR_STATES_ERROR, { currentError: "No microphone found" });
    await flush();
    expect(motion()).toBe("recoil");
    expect(mono()).toBe(1);
    expect(posture()).toBe("words");
    expect(screen.getByTestId("orb-caption").textContent).toBe("No microphone found");
    expect(screen.getByTestId("orb-caption").getAttribute("data-tone")).toBe("error");
  });

  it("holds a green ember for an answer nobody has read, and opens it on a click", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("what time is it")];
    await send(UI.BAR_STATES_LOADING);
    chat.messages = [user("what time is it"), assistant()];
    await send(UI.BAR_STATES_DEFAULT);
    await flush();
    expect(motion()).toBe("ember");
    expect(hue()).toBe(110);
    expect(posture()).toBe("orb");

    fireEvent.click(orb());
    await flush();
    expect(posture()).toBe("sheet");
    expect(screen.getByTestId("answer").textContent).toBe("It is 3pm.");
    // The question is in the sheet too, so a misheard query is recoverable.
    expect(screen.getByTestId("orb-sheet-question").textContent).toBe("what time is it");
    // Read: the ember lets go.
    expect(hue()).toBe(0);

    fireEvent.click(orb());
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(posture()).toBe("orb");
  });

  it("opens the sheet by itself for an answer that cannot be spoken", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("what is playing")];
    await send(UI.BAR_STATES_LOADING);
    chat.messages = [user("what is playing"), assistant({ content: "<NowPlayingCard />" })];
    await send(UI.BAR_STATES_DEFAULT);
    await flush();
    expect(posture()).toBe("sheet");
  });

  it("lets go of an unread answer once its clock runs out", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("what time is it")];
    await send(UI.BAR_STATES_LOADING);
    chat.messages = [user("what time is it"), assistant()];
    await send(UI.BAR_STATES_DEFAULT);
    await flush();
    expect(hue()).toBe(110);
    await act(async () => {
      vi.advanceTimersByTime(HELD_MS + 500);
    });
    expect(hue()).toBe(0);
  });

  it("clears everything the moment you speak again", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("what time is it"), assistant()];
    await send(UI.BAR_STATES_LOADING);
    await send(UI.BAR_STATES_DEFAULT);
    await flush();
    fireEvent.click(orb());
    await flush();
    expect(posture()).toBe("sheet");
    await send(UI.BAR_STATES_LISTENING);
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(posture()).toBe("orb");
  });

  it("does not linger over an answer that was already in the history", async () => {
    chat.messages = [user("old question"), assistant({ messageId: "old" })];
    render(<ShaderOrbBar />);
    await flush();
    expect(posture()).toBe("orb");
    expect(hue()).toBe(0);
  });

  it("opens the composer when Rust says you are typing, and submits through Rust", async () => {
    render(<ShaderOrbBar />);
    await send(UI.BAR_STATES_INPUT);
    await flush();
    expect(posture()).toBe("words");
    expect(motion()).toBe("wake");
    const field = screen.getByLabelText("Ask Juno");
    fireEvent.change(field, { target: { value: "open my notes" } });
    expect(invoke).toHaveBeenCalledWith(
      "ui_handle_interaction",
      interaction(UI.INTERACTION_TYPES_INPUT_CHANGE),
    );
    fireEvent.submit(field);
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction(UI.INTERACTION_TYPES_SUBMIT));
  });

  it("asks Rust to open the composer when there is nothing to read", async () => {
    render(<ShaderOrbBar />);
    invoke.mockClear();
    fireEvent.click(orb());
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction(UI.INTERACTION_TYPES_CLICK));
  });

  it("sends Escape to Rust while it works, and only settles itself when it is not", async () => {
    render(<ShaderOrbBar />);
    chat.messages = [user("a long job")];
    await send(UI.BAR_STATES_LOADING);
    invoke.mockClear();
    fireEvent.keyDown(document, { key: "Escape" });
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("ui_handle_interaction", interaction(UI.INTERACTION_TYPES_ESCAPE));

    chat.messages = [user("a long job"), assistant()];
    await send(UI.BAR_STATES_DEFAULT);
    await flush();
    fireEvent.click(orb());
    await flush();
    expect(posture()).toBe("sheet");
    invoke.mockClear();
    fireEvent.keyDown(document, { key: "Escape" });
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(posture()).toBe("orb");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("turns steadily while Juno drives the cursor, and opens the sheet when it asks", async () => {
    render(<ShaderOrbBar />);
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_STATE)?.({ active: true, app: "Safari" });
    });
    await flush();
    expect(motion()).toBe("turn");
    await act(async () => {
      eventHandlers.get(EVENTS.INPUT_CONTROL_REQUEST)?.({});
    });
    await flush();
    expect(posture()).toBe("sheet");
  });

  it("grows the window before the panel shows, and shrinks it only after it has gone", async () => {
    render(<ShaderOrbBar />);
    await flush();
    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_ERROR, { currentError: "boom" });
    await flush();
    const grown = resizeWindowIfChanged.mock.calls.at(-1)?.[0];
    expect(grown?.height).toBeGreaterThan(STAGE);
    expect(posture()).toBe("words");

    resizeWindowIfChanged.mockClear();
    await send(UI.BAR_STATES_DEFAULT);
    // The panel is gone at once; the window waits for its fade.
    expect(posture()).toBe("orb");
    expect(resizeWindowIfChanged).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SHRINK_DELAY_MS + 10);
    });
    expect(resizeWindowIfChanged).toHaveBeenLastCalledWith({ width: STAGE, height: STAGE });
  });

  it("never shows a status word or your transcript, in any state", async () => {
    render(<ShaderOrbBar />);
    for (const state of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_ALWAYS_LISTENING,
      UI.BAR_STATES_DICTATION_READY,
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_DICTATING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_STOPPING,
    ]) {
      await send(state, { transcriptionText: "the quick brown fox", spokenText: "the answer is 42" });
      await flush();
      expect(bar().textContent, state).toBe("");
    }
  });
});
