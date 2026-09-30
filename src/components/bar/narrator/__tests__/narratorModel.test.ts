import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  BAR_WIDTH,
  SHADOW_PAD,
  SHEET_MAX_HEIGHT,
  SHEET_MIN_HEIGHT,
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  SYSTEM_RED,
  answerKey,
  dotFor,
  drainShouldRun,
  firstSentence,
  hasComponent,
  latestTurn,
  lineFor,
  posture,
  railFor,
  sheetHeightFor,
  sheetWanted,
  windowSize,
  type LineInput,
  type Turn,
} from "../narratorModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

const emptyTurn: Turn = { question: "", steps: [], answer: null };

const step = (
  description: string,
  extra: Partial<ChatMessage> = {},
): ChatMessage => ({ role: "tool_call_request", content: description, tool_name: "browser", ...extra });

function line(overrides: Partial<LineInput> & { state: string }) {
  return lineFor({
    transcriptionText: "",
    spokenText: "",
    currentError: null,
    question: "",
    turn: emptyTurn,
    answerLine: "",
    ...overrides,
  });
}

describe("posture", () => {
  it("rests, composes, listens, or narrates a turn", () => {
    const at = (state: string) => posture({ state });
    expect(at(UI.BAR_STATES_DEFAULT)).toBe("rest");
    expect(at(UI.BAR_STATES_DICTATION_READY)).toBe("rest");
    expect(at(UI.BAR_STATES_SHRINKING)).toBe("rest");
    expect(at(UI.BAR_STATES_EXPANDING)).toBe("compose");
    expect(at(UI.BAR_STATES_INPUT)).toBe("compose");
    expect(at(UI.BAR_STATES_LISTENING)).toBe("listen");
    expect(at(UI.BAR_STATES_DICTATING)).toBe("listen");
    // Always listening rests: the dot says it is listening, the line keeps the last turn.
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING)).toBe("rest");
    for (const s of [
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(at(s)).toBe("turn");
    }
  });

  it("maps every state Rust can send to a posture", () => {
    for (const state of EVERY_STATE) {
      expect(["rest", "compose", "listen", "turn"]).toContain(posture({ state }));
    }
  });

  it("driving the cursor is narrated, whatever Rust says", () => {
    expect(posture({ state: UI.BAR_STATES_INPUT, driving: true })).toBe("turn");
  });
});

describe("sizes", () => {
  it("the strip never changes width, so the window only ever changes height", () => {
    expect(windowSize().width).toBe(BAR_WIDTH + 2 * SHADOW_PAD);
    expect(windowSize(200).width).toBe(windowSize().width);
    expect(windowSize(200).height).toBe(windowSize().height + 200);
  });

  it("the sheet follows its content in steps, within its range", () => {
    expect(sheetHeightFor(10)).toBe(SHEET_MIN_HEIGHT);
    expect(sheetHeightFor(150)).toBe(152);
    expect(sheetHeightFor(153)).toBe(160);
    expect(sheetHeightFor(9000)).toBe(SHEET_MAX_HEIGHT);
  });
});

describe("dot", () => {
  it("has a colour and motion for every state, in both themes", () => {
    for (const state of EVERY_STATE) {
      for (const theme of ["light", "dark"] as const) {
        const d = dotFor(state, theme);
        expect(d.color).toMatch(/^#/);
        expect(d.opacity).toBeGreaterThan(0);
      }
    }
  });

  it("uses ink for rest so it reads on a light strip too", () => {
    expect(dotFor(UI.BAR_STATES_DEFAULT, "dark").color).toBe("#FFFFFF");
    expect(dotFor(UI.BAR_STATES_DEFAULT, "light").color).toBe("#1C1C1E");
  });

  it("tells the state by colour and motion, not by icon", () => {
    expect(dotFor(UI.BAR_STATES_LISTENING, "dark")).toMatchObject({ color: SYSTEM_BLUE, motion: "breathe" });
    expect(dotFor(UI.BAR_STATES_DICTATING, "dark")).toMatchObject({ color: SYSTEM_GREEN, motion: "breathe" });
    expect(dotFor(UI.BAR_STATES_LOADING, "dark")).toMatchObject({ color: SYSTEM_BLUE, motion: "orbit" });
    expect(dotFor(UI.BAR_STATES_SPEAKING, "dark")).toMatchObject({ color: SYSTEM_BLUE, motion: "ripple" });
    expect(dotFor(UI.BAR_STATES_ERROR, "dark")).toMatchObject({ color: SYSTEM_RED, motion: "shake" });
    expect(dotFor(UI.BAR_STATES_SUCCESS, "dark")).toMatchObject({ color: SYSTEM_GREEN, motion: "flash" });
    expect(dotFor(UI.BAR_STATES_INPUT, "dark", true)).toMatchObject({ color: SYSTEM_BLUE, motion: "orbit" });
  });
});

describe("the current turn", () => {
  it("is the last question and every step and answer since it", () => {
    const messages: ChatMessage[] = [
      { role: "user", content: "old", timestamp: 1 },
      { role: "assistant", content: "old answer", messageId: "a0", timestamp: 2 },
      { role: "user", content: "Open the page", timestamp: 3 },
      step("open the page in Safari", { success: true }),
      step("read the page", {}),
      { role: "assistant", content: "It says hello.", messageId: "a1", timestamp: 4 },
    ];
    const turn = latestTurn(messages);
    expect(turn.question).toBe("Open the page");
    expect(turn.steps.map((s) => [s.description, s.status])).toEqual([
      ["open the page in Safari", "done"],
      ["read the page", "running"],
    ]);
    expect(turn.answer?.content).toBe("It says hello.");
    expect(answerKey(turn)).toBe("a1");
  });

  it("knows a pending, denied and failed step", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      step("a", { approval_state: "pending", tool_id: "t1" }),
      step("b", { approval_state: "denied", tool_id: "t2" }),
      step("c", { success: false }),
    ]);
    expect(turn.steps.map((s) => s.status)).toEqual(["pending", "denied", "failed"]);
  });

  it("ignores notices and empty streaming bubbles as answers", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Welcome", notice: true, timestamp: 2 },
      { role: "assistant", content: "", isStreaming: true, messageId: "m", timestamp: 3 },
    ]);
    expect(turn.answer).toBeNull();
  });
});

describe("the answer line", () => {
  it("is the first sentence, with markdown and components dropped", () => {
    expect(firstSentence("**Done.** The draft is with Maya.")).toBe("Done.");
    expect(firstSentence("Here is the list:\n\n<TaskList items={[]} />\n\nThat is all.")).toBe("Here is the list: That is all.");
    expect(firstSentence("Sent! The draft is with Maya.")).toBe("Sent!");
    expect(firstSentence("Sunny, 72 degrees")).toBe("Sunny, 72 degrees");
    expect(firstSentence("<Weather city=\"Charlotte\" />")).toBe("");
  });

  it("opens the sheet only when the line cannot hold the answer", () => {
    expect(sheetWanted({ visibleText: "Done.", streaming: false })).toBe(false);
    expect(sheetWanted({ visibleText: "Done. Sent to Maya.", streaming: false })).toBe(true);
    expect(sheetWanted({ visibleText: '<Weather city="x" />', streaming: false })).toBe(true);
    expect(sheetWanted({ visibleText: "", streaming: true })).toBe(false);
    expect(hasComponent("plain <b>html</b>")).toBe(false);
  });
});

describe("the line, every state", () => {
  it("runs your words along the line while you speak, green", () => {
    expect(line({ state: UI.BAR_STATES_LISTENING })).toEqual([
      { kind: "note", text: "Listening", tone: "dim", bead: false },
    ]);
    expect(line({ state: UI.BAR_STATES_LISTENING, transcriptionText: "Send the draft" })).toEqual([
      { kind: "question", text: "Send the draft", tone: "green", bead: false },
    ]);
    expect(
      line({ state: UI.BAR_STATES_DICTATING, transcriptionText: "Send the", transcriptionProvisional: true })[0].tone,
    ).toBe("provisional");
    expect(line({ state: UI.BAR_STATES_ALWAYS_LISTENING })[0].text).toBe("Listening for “hey Juno”");
    // With a turn to remember, always listening keeps it on the line like rest does.
    expect(line({ state: UI.BAR_STATES_ALWAYS_LISTENING, question: "Q", answerLine: "A." }).map((s) => s.kind)).toEqual([
      "question",
      "answer",
    ]);
    expect(line({ state: UI.BAR_STATES_TRANSCRIBING, transcriptionText: "Send it" })[0]).toMatchObject({
      text: "Send it",
      tone: "dim",
    });
  });

  it("says nothing at rest, and nothing while composing", () => {
    expect(line({ state: UI.BAR_STATES_DEFAULT })).toEqual([]);
    expect(line({ state: UI.BAR_STATES_DICTATION_READY })).toEqual([]);
    expect(line({ state: UI.BAR_STATES_SHRINKING })).toEqual([]);
    expect(line({ state: UI.BAR_STATES_INPUT })).toEqual([]);
    expect(line({ state: UI.BAR_STATES_EXPANDING })).toEqual([]);
  });

  it("keeps the last turn on the line at rest, dimmed", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "A.", messageId: "m", timestamp: 2 },
    ]);
    expect(line({ state: UI.BAR_STATES_DEFAULT, question: "Q", turn, answerLine: "A." })).toEqual([
      { kind: "question", text: "Q", tone: "dim", bead: false },
      { kind: "answer", text: "A.", tone: "plain", bead: true, live: false },
    ]);
  });

  it("narrates sending and thinking before there is a step", () => {
    expect(line({ state: UI.BAR_STATES_SUBMITTING, question: "Q" })).toEqual([
      { kind: "question", text: "Q", tone: "plain", bead: false },
      { kind: "note", text: "Sending", tone: "dim", bead: true, live: true },
    ]);
    expect(line({ state: UI.BAR_STATES_LOADING, question: "Q" })[1]).toMatchObject({ text: "Thinking", live: true });
    expect(line({ state: UI.BAR_STATES_AGENT_RESPONDING, question: "Q" })[1]).toMatchObject({ text: "Thinking" });
  });

  it("puts a bead on the line for each step, and only the newest keeps its words", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      step("open Safari", { success: true }),
      step("read the page", {}),
    ]);
    const segs = line({ state: UI.BAR_STATES_LOADING, question: "Q", turn });
    expect(segs.map((s) => [s.kind, s.collapsed ?? false, s.live ?? false])).toEqual([
      ["question", false, false],
      ["step", true, false],
      ["step", false, true],
    ]);
    expect(segs[2].text).toBe("read the page");
  });

  it("a done step keeps its words until something newer arrives", () => {
    const turn = latestTurn([{ role: "user", content: "Q", timestamp: 1 }, step("open Safari", { success: true })]);
    const segs = line({ state: UI.BAR_STATES_LOADING, question: "Q", turn });
    expect(segs).toHaveLength(2);
    expect(segs[1]).toMatchObject({ kind: "step", text: "open Safari", collapsed: false, live: false });
    // No "Thinking" on top of it.
    expect(segs.some((s) => s.kind === "note")).toBe(false);
  });

  it("stops the line at a step waiting on Allow or Don't", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      step("send the email", { approval_state: "pending", tool_id: "t1" }),
    ]);
    const segs = line({ state: UI.BAR_STATES_LOADING, question: "Q", turn });
    expect(segs[1]).toMatchObject({ kind: "approval", text: "Juno wants to send the email", live: true });
    expect(segs[1].step?.toolId).toBe("t1");
  });

  it("ends the line with the answer, live while it streams", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      step("open Safari", { success: true }),
      { role: "assistant", content: "Sunny.", messageId: "m", isStreaming: true, timestamp: 2 },
    ]);
    const segs = line({ state: UI.BAR_STATES_AGENT_RESPONDING, question: "Q", turn, answerLine: "Sunny." });
    expect(segs.map((s) => s.kind)).toEqual(["question", "step", "answer"]);
    expect(segs[1].collapsed).toBe(true);
    expect(segs[2]).toMatchObject({ text: "Sunny.", tone: "live", live: true });
  });

  it("shows a spoken-only answer as the line, dimmed", () => {
    expect(
      line({ state: UI.BAR_STATES_SPEAKING, question: "Q", spokenText: "Done.", spokenOnly: true, answerLine: "" })[1],
    ).toMatchObject({ kind: "answer", text: "Done.", tone: "dim" });
  });

  it("says Done when a turn ends with no answer, and Stopping while it stops", () => {
    expect(line({ state: UI.BAR_STATES_FINISHING, question: "Q" })[1]).toMatchObject({ text: "Done", live: false });
    expect(line({ state: UI.BAR_STATES_SUCCESS })).toEqual([{ kind: "note", text: "Done", tone: "dim", bead: false, live: false }]);
    expect(line({ state: UI.BAR_STATES_STOPPING, question: "Q" })).toEqual([
      { kind: "note", text: "Stopping", tone: "dim", bead: false },
    ]);
  });

  it("turns red at the bead that failed, with the message", () => {
    const turn = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      step("open Safari", { success: true }),
      step("read the page", { success: false }),
    ]);
    const segs = line({ state: UI.BAR_STATES_ERROR, question: "Q", turn, currentError: "Page timed out" });
    expect(segs[1].collapsed).toBe(true);
    expect(segs[1].failed).toBeFalsy();
    expect(segs[2]).toMatchObject({ kind: "step", failed: true, tone: "error", text: "read the page: Page timed out" });
    expect(segs).toHaveLength(3);
  });

  it("adds a red bead when the error is not a step's", () => {
    expect(line({ state: UI.BAR_STATES_ERROR, question: "Q", currentError: "Connection unavailable" })[1]).toEqual({
      kind: "error",
      text: "Connection unavailable",
      tone: "error",
      bead: true,
      failed: true,
    });
    expect(line({ state: UI.BAR_STATES_ERROR })).toEqual([
      { kind: "error", text: "Something went wrong", tone: "error", bead: false, failed: true },
    ]);
  });

  it("narrates Juno driving the cursor as the live bead", () => {
    const segs = line({ state: UI.BAR_STATES_LOADING, question: "Q", drivingLabel: "using the mouse in Safari" });
    expect(segs[1]).toMatchObject({ kind: "driving", text: "using the mouse in Safari", live: true });
  });
});

describe("the rail", () => {
  const rail = (state: string, extra: Partial<Parameters<typeof railFor>[0]> = {}) =>
    railFor({ state, audioLevel: 0, segments: [], answerLength: 0, streaming: false, drain: null, ...extra });

  it("is your voice level while you speak", () => {
    expect(rail(UI.BAR_STATES_LISTENING, { audioLevel: 0.6 })).toEqual({ kind: "meter", level: 0.6 });
    expect(rail(UI.BAR_STATES_DICTATING, { audioLevel: 3 })).toEqual({ kind: "meter", level: 1 });
  });

  it("is empty at rest, composing, always listening and stopping", () => {
    for (const s of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_DICTATION_READY,
      UI.BAR_STATES_SHRINKING,
      UI.BAR_STATES_INPUT,
      UI.BAR_STATES_EXPANDING,
      UI.BAR_STATES_ALWAYS_LISTENING,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(rail(s)).toEqual({ kind: "empty" });
    }
  });

  it("reaches the live bead while a step runs", () => {
    const segments = line({
      state: UI.BAR_STATES_LOADING,
      question: "Q",
      turn: latestTurn([{ role: "user", content: "Q", timestamp: 1 }, step("a", { success: true }), step("b", {})]),
    });
    expect(rail(UI.BAR_STATES_LOADING, { segments })).toEqual({ kind: "toBead", index: 2 });
  });

  it("starts as a sliver before there is anything to point at", () => {
    expect(rail(UI.BAR_STATES_TRANSCRIBING)).toEqual({ kind: "streaming", fraction: 0 });
  });

  it("fills past the answer bead as the answer streams, then is full", () => {
    const segments = line({
      state: UI.BAR_STATES_AGENT_RESPONDING,
      question: "Q",
      turn: latestTurn([
        { role: "user", content: "Q", timestamp: 1 },
        { role: "assistant", content: "Sunny.", messageId: "m", isStreaming: true, timestamp: 2 },
      ]),
      answerLine: "Sunny.",
    });
    const r = rail(UI.BAR_STATES_AGENT_RESPONDING, { segments, streaming: true, answerLength: 240 });
    expect(r.kind).toBe("streaming");
    if (r.kind === "streaming") expect(r.fraction).toBeCloseTo(0.63, 1);
    expect(rail(UI.BAR_STATES_AGENT_RESPONDING, { segments })).toEqual({ kind: "full" });
    expect(rail(UI.BAR_STATES_FINISHING)).toEqual({ kind: "full" });
    expect(rail(UI.BAR_STATES_SPEAKING)).toEqual({ kind: "full" });
  });

  it("drains once the answer is done, and turns red where it failed", () => {
    expect(rail(UI.BAR_STATES_DEFAULT, { drain: 0.4 })).toEqual({ kind: "drain", progress: 0.4 });
    const segments = line({ state: UI.BAR_STATES_ERROR, question: "Q", currentError: "No" });
    expect(rail(UI.BAR_STATES_ERROR, { segments })).toEqual({ kind: "failed", index: 1, fraction: 0.5 });
    expect(rail(UI.BAR_STATES_ERROR)).toEqual({ kind: "failed", index: null, fraction: 0.5 });
  });

  it("has a shape for every state Rust can send", () => {
    for (const state of EVERY_STATE) {
      expect(rail(state).kind).toBeTruthy();
    }
  });
});

describe("the drain", () => {
  it("runs only once the answer is complete and nothing is waiting", () => {
    expect(drainShouldRun({ hasAnswer: true, streaming: false, working: false, approvalPending: false })).toBe(true);
    expect(drainShouldRun({ hasAnswer: false, streaming: false, working: false, approvalPending: false })).toBe(false);
    expect(drainShouldRun({ hasAnswer: true, streaming: true, working: false, approvalPending: false })).toBe(false);
    expect(drainShouldRun({ hasAnswer: true, streaming: false, working: true, approvalPending: false })).toBe(false);
    expect(drainShouldRun({ hasAnswer: true, streaming: false, working: false, approvalPending: true })).toBe(false);
  });
});
