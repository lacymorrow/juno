import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  INSIDE_MAX_CHARS,
  NARROW_WINDOW,
  RING_BOX,
  SHADOW_PAD,
  SHEET_MAX_CONTENT,
  SHEET_WIDTH,
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  TICK_COUNT,
  answerKey,
  answerText,
  captionFor,
  gapAngle,
  insideFontSize,
  latestTurn,
  lingerShouldRun,
  placementFor,
  plainInside,
  ringFor,
  sheetHeight,
  tickAngle,
  tickAngles,
  windowFor,
  type RingInput,
} from "../haloModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

const base: RingInput = {
  state: UI.BAR_STATES_DEFAULT,
  audioLevel: 0,
  completedTools: 0,
  runningTool: false,
  approvalPending: false,
  driving: false,
  closing: false,
  lingerProgress: null,
};

const ring = (state: string, extra: Partial<RingInput> = {}) => ringFor({ ...base, state, ...extra });

describe("the ring", () => {
  it("rests thin and quiet, and gives every state Rust can send a verb", () => {
    expect(ring(UI.BAR_STATES_DEFAULT)).toMatchObject({ verb: "rest", opacity: 0.3, breathe: false });
    for (const state of EVERY_STATE) {
      expect(["rest", "fill", "travel", "pulse", "close", "gap", "drain"]).toContain(ring(state).verb);
    }
  });

  it("fills with your level in blue while listening, green while dictating", () => {
    expect(ring(UI.BAR_STATES_LISTENING, { audioLevel: 0.4 })).toMatchObject({
      verb: "fill",
      color: SYSTEM_BLUE,
      level: 0.4,
    });
    expect(ring(UI.BAR_STATES_DICTATING, { audioLevel: 0.7 })).toMatchObject({
      verb: "fill",
      color: SYSTEM_GREEN,
      level: 0.7,
    });
    // A level out of range is clamped, never drawn past the circle.
    expect(ring(UI.BAR_STATES_LISTENING, { audioLevel: 3 }).level).toBe(1);
    expect(ring(UI.BAR_STATES_LISTENING, { audioLevel: Number.NaN }).level).toBe(0);
  });

  it("reads full while the sentence is written down", () => {
    expect(ring(UI.BAR_STATES_TRANSCRIBING)).toMatchObject({ verb: "fill", color: SYSTEM_GREEN, level: 1 });
  });

  it("breathes slowly when always listening, and is green when dictation is ready", () => {
    expect(ring(UI.BAR_STATES_ALWAYS_LISTENING)).toMatchObject({ verb: "rest", color: SYSTEM_BLUE, breathe: true });
    expect(ring(UI.BAR_STATES_DICTATION_READY)).toMatchObject({ verb: "rest", color: SYSTEM_GREEN });
  });

  it("travels while Juno works, and pauses at the next tick while a tool runs", () => {
    for (const s of [UI.BAR_STATES_SUBMITTING, UI.BAR_STATES_LOADING, UI.BAR_STATES_AGENT_RESPONDING]) {
      expect(ring(s)).toMatchObject({ verb: "travel", holdAt: null });
    }
    expect(ring(UI.BAR_STATES_LOADING, { completedTools: 2, runningTool: true }).holdAt).toBe(tickAngle(2));
    expect(ring(UI.BAR_STATES_LOADING, { completedTools: 0, approvalPending: true }).holdAt).toBe(0);
    expect(ring(UI.BAR_STATES_STOPPING)).toMatchObject({ verb: "travel", slow: true });
  });

  it("pulses while Juno speaks", () => {
    expect(ring(UI.BAR_STATES_SPEAKING).verb).toBe("pulse");
  });

  it("closes fully in green when the turn is done, and keeps the close while asked to", () => {
    expect(ring(UI.BAR_STATES_FINISHING)).toMatchObject({ verb: "close", color: SYSTEM_GREEN, level: 1 });
    expect(ring(UI.BAR_STATES_SUCCESS).verb).toBe("close");
    // Rust moved on to Default 300ms in; the ring still shows its close.
    expect(ring(UI.BAR_STATES_DEFAULT, { closing: true }).verb).toBe("close");
  });

  it("breaks red where it failed", () => {
    expect(ring(UI.BAR_STATES_ERROR).verb).toBe("gap");
    expect(gapAngle(0)).toBe(0);
    expect(gapAngle(3)).toBe(tickAngle(3));
  });

  it("drains as the clock while a finished answer lingers", () => {
    expect(ring(UI.BAR_STATES_DEFAULT, { lingerProgress: 0.5 })).toMatchObject({ verb: "drain", level: 0.5 });
    // No answer on stage: plain rest.
    expect(ring(UI.BAR_STATES_DEFAULT, { lingerProgress: null }).verb).toBe("rest");
  });

  it("travels in blue while Juno drives the cursor, whatever Rust says", () => {
    expect(ring(UI.BAR_STATES_INPUT, { driving: true })).toMatchObject({ verb: "travel", color: SYSTEM_BLUE });
  });

  it("dims to nothing much while typing or shrinking", () => {
    expect(ring(UI.BAR_STATES_INPUT).opacity).toBeGreaterThan(ring(UI.BAR_STATES_DEFAULT).opacity);
    expect(ring(UI.BAR_STATES_SHRINKING).opacity).toBeLessThan(ring(UI.BAR_STATES_DEFAULT).opacity);
  });
});

describe("ticks", () => {
  it("sit like the hours on a clock, one per finished tool, at most twelve", () => {
    expect(tickAngle(0)).toBe(0);
    expect(tickAngle(3)).toBe(90);
    expect(tickAngle(12)).toBe(0);
    expect(tickAngles(0)).toEqual([]);
    expect(tickAngles(3)).toEqual([0, 30, 60]);
    expect(tickAngles(40)).toHaveLength(TICK_COUNT);
  });
});

describe("the caption", () => {
  const input = (state: string, extra: Record<string, unknown> = {}) =>
    captionFor({
      state,
      transcriptionText: "",
      spokenText: "",
      currentError: null,
      question: "",
      ...extra,
    });

  it("shows your words as you speak, and a quiet label before any arrive", () => {
    expect(input(UI.BAR_STATES_LISTENING)).toEqual({ text: "Listening", tone: "dim" });
    expect(input(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" })).toEqual({ text: "Send it", tone: "live" });
    expect(input(UI.BAR_STATES_DICTATING, { transcriptionText: "Send", transcriptionProvisional: true })).toEqual({
      text: "Send",
      tone: "provisional",
    });
    expect(input(UI.BAR_STATES_ALWAYS_LISTENING)?.text).toBe("Listening for “hey Juno”");
    expect(input(UI.BAR_STATES_TRANSCRIBING, { transcriptionText: "Send it" })).toEqual({ text: "Send it", tone: "dim" });
  });

  it("keeps what you asked in view while Juno works, then the running tool", () => {
    expect(input(UI.BAR_STATES_SUBMITTING, { question: "Q" })).toEqual({ text: "Q", tone: "dim" });
    expect(input(UI.BAR_STATES_LOADING, { question: "Q", runningTool: "Checking your calendar" })).toEqual({
      text: "Checking your calendar",
      tone: "plain",
    });
    expect(input(UI.BAR_STATES_AGENT_RESPONDING, { question: "Q", answerShowing: true })).toBeNull();
    // The approval row says what; the question would only repeat it.
    expect(input(UI.BAR_STATES_LOADING, { question: "Q", approvalPending: true })).toBeNull();
  });

  it("says nothing when the ring closing is the word", () => {
    expect(input(UI.BAR_STATES_FINISHING)).toBeNull();
    expect(input(UI.BAR_STATES_SUCCESS)).toBeNull();
    expect(input(UI.BAR_STATES_DEFAULT)).toBeNull();
    expect(input(UI.BAR_STATES_INPUT)).toBeNull();
  });

  it("carries the error, the spoken sentence, stopping, and the driving label", () => {
    expect(input(UI.BAR_STATES_ERROR, { currentError: "No connection" })).toEqual({ text: "No connection", tone: "error" });
    expect(input(UI.BAR_STATES_ERROR)?.text).toBe("Something went wrong");
    expect(input(UI.BAR_STATES_SPEAKING, { spokenText: "Hello" })).toEqual({ text: "Hello", tone: "dim" });
    expect(input(UI.BAR_STATES_SPEAKING, { spokenText: "Hello", answerShowing: true })).toBeNull();
    expect(input(UI.BAR_STATES_STOPPING)?.text).toBe("Stopping");
    expect(input(UI.BAR_STATES_DEFAULT, { drivingLabel: "using the mouse in Safari" })).toEqual({
      text: "using the mouse in Safari",
      tone: "plain",
    });
  });
});

describe("where the answer goes", () => {
  it("puts a number, a word, a time, a yes inside the ring once it is complete", () => {
    expect(placementFor({ text: "72°F", streaming: false })).toBe("inside");
    expect(placementFor({ text: "Yes", streaming: false })).toBe("inside");
    expect(placementFor({ text: "3:42 pm", streaming: false })).toBe("inside");
    expect(placementFor({ text: "**42 minutes**", streaming: false })).toBe("inside");
    expect(placementFor({ text: "a".repeat(INSIDE_MAX_CHARS), streaming: false })).toBe("inside");
  });

  it("does not decide inside while the answer is still growing", () => {
    expect(placementFor({ text: "72", streaming: true })).toBe("none");
    expect(placementFor({ text: "a".repeat(INSIDE_MAX_CHARS + 1), streaming: true })).toBe("below");
  });

  it("unfolds anything long, a list, or a component below", () => {
    expect(placementFor({ text: "The meeting moved to Friday at three.", streaming: false })).toBe("below");
    expect(placementFor({ text: "Done.\n\nNext.", streaming: false })).toBe("below");
    expect(placementFor({ text: "- one\n- two", streaming: false })).toBe("below");
    expect(placementFor({ text: '<TaskSummaryCard title="Sent" />', streaming: false })).toBe("below");
    expect(placementFor({ text: "", streaming: false })).toBe("none");
  });

  it("sizes the type to what fits: a number big, a phrase small", () => {
    expect(insideFontSize("42")).toBe(26);
    expect(insideFontSize("3:42 pm")).toBe(18);
    expect(insideFontSize("Sent to Maya now")).toBe(13);
    expect(plainInside("**Yes**")).toBe("Yes");
  });
});

describe("the window", () => {
  it("is the disc plus its margin when nothing is under the ring", () => {
    expect(windowFor(false)).toEqual(NARROW_WINDOW);
    expect(NARROW_WINDOW.width).toBe(RING_BOX + 2 * SHADOW_PAD);
  });

  it("grows around the ring and downward from its centre when the sheet opens", () => {
    const w = windowFor(true, 40);
    expect(w.width).toBe(SHEET_WIDTH + 2 * SHADOW_PAD);
    expect(w.height).toBe(SHADOW_PAD + RING_BOX / 2 + sheetHeight(40) + SHADOW_PAD);
    // The ring's centre is at the same offset from the top in both windows.
    expect(windowFor(false).height / 2).toBe(SHADOW_PAD + RING_BOX / 2);
  });

  it("follows the sheet's content in steps, within its range", () => {
    expect(sheetHeight(0)).toBe(sheetHeight(0));
    expect(sheetHeight(41)).toBe(sheetHeight(48));
    expect(sheetHeight(9000)).toBe(sheetHeight(SHEET_MAX_CONTENT));
  });
});

describe("the linger", () => {
  it("counts only once a finished answer is on stage and nothing is pending", () => {
    const ok = { answerShowing: true, streaming: false, working: false, approvalPending: false };
    expect(lingerShouldRun(ok)).toBe(true);
    expect(lingerShouldRun({ ...ok, answerShowing: false })).toBe(false);
    expect(lingerShouldRun({ ...ok, streaming: true })).toBe(false);
    expect(lingerShouldRun({ ...ok, working: true })).toBe(false);
    expect(lingerShouldRun({ ...ok, approvalPending: true })).toBe(false);
  });
});

describe("the current turn", () => {
  const user = (content: string): ChatMessage => ({ role: "user", content, timestamp: 1 });
  const tool = (content: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
    role: "tool_call_request",
    content,
    tool_name: "t",
    ...extra,
  });

  it("counts finished tools as ticks and names the one still running", () => {
    const turn = latestTurn([
      user("Q"),
      tool("Checking your calendar", { success: true }),
      tool("Reading the invite", { success: false }),
      tool("Opening Safari"),
    ]);
    expect(turn.question).toBe("Q");
    expect(turn.completedTools).toBe(2);
    expect(turn.runningTool).toBe("Opening Safari");
    expect(turn.answer).toBeNull();
  });

  it("starts fresh at the last question", () => {
    const turn = latestTurn([
      user("Old"),
      tool("Old tool", { success: true }),
      { role: "assistant", content: "Old answer", messageId: "a" },
      user("New"),
    ]);
    expect(turn.question).toBe("New");
    expect(turn.completedTools).toBe(0);
    expect(turn.answer).toBeNull();
  });

  it("finds the approval and the answer, and skips notices and denied tools", () => {
    const turn = latestTurn([
      user("Q"),
      tool("open Safari", { tool_id: "t1", approval_state: "pending" }),
      tool("delete it", { approval_state: "denied" }),
      { role: "assistant", content: "Setup complete", notice: true },
      { role: "assistant", content: "", messageId: "m1", tts_metadata: { has_spoken_content: true, tts_parts: ["Done."], total_spoken_text: "Done." } },
    ]);
    expect(turn.approval?.tool_id).toBe("t1");
    expect(turn.runningTool).toBeNull();
    expect(answerKey(turn)).toBe("m1");
    expect(answerText(turn.answer)).toEqual({ visible: "", spoken: "Done." });
  });
});
