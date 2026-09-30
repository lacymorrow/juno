import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  ISLAND_SIZES,
  SHADOW_PAD,
  answerKey,
  cardHeightFor,
  dotFor,
  islandSize,
  latestTurn,
  posture,
  ringShouldRun,
  windowSize,
  wordsFor,
} from "../islandModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

describe("posture", () => {
  it("rests as a capsule, opens an ear to speak, a line to type, a status line to work", () => {
    const at = (state: string) => posture({ state, cardOpen: false });
    expect(at(UI.BAR_STATES_DEFAULT)).toBe("capsule");
    expect(at(UI.BAR_STATES_DICTATION_READY)).toBe("capsule");
    expect(at(UI.BAR_STATES_SHRINKING)).toBe("capsule");
    expect(at(UI.BAR_STATES_LISTENING)).toBe("ear");
    expect(at(UI.BAR_STATES_DICTATING)).toBe("ear");
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING)).toBe("ear");
    expect(at(UI.BAR_STATES_INPUT)).toBe("line");
    expect(at(UI.BAR_STATES_EXPANDING)).toBe("line");
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
      expect(at(s)).toBe("status");
    }
  });

  it("maps every state Rust can send to a posture", () => {
    for (const state of EVERY_STATE) {
      expect(["capsule", "ear", "line", "status", "card"]).toContain(posture({ state, cardOpen: false }));
    }
  });

  it("an open card wins over everything, driving wins over the line", () => {
    expect(posture({ state: UI.BAR_STATES_INPUT, cardOpen: true })).toBe("card");
    expect(posture({ state: UI.BAR_STATES_LISTENING, cardOpen: true })).toBe("card");
    expect(posture({ state: UI.BAR_STATES_INPUT, cardOpen: false, driving: true })).toBe("status");
  });
});

describe("sizes", () => {
  it("ear and status share a size so the island never lurches when the mic closes", () => {
    expect(islandSize("ear")).toEqual(islandSize("status"));
  });

  it("the capsule is a mark, not a bar", () => {
    const c = islandSize("capsule");
    expect(c.width).toBeLessThan(100);
    expect(c.height).toBeLessThan(32);
    expect(c.radius).toBe(c.height / 2);
  });

  it("the card follows its content in steps, within its range", () => {
    expect(cardHeightFor(10)).toBe(ISLAND_SIZES.card.minHeight);
    expect(cardHeightFor(150)).toBe(152);
    expect(cardHeightFor(153)).toBe(160);
    expect(cardHeightFor(9000)).toBe(ISLAND_SIZES.card.maxHeight);
    expect(islandSize("card", 150).height).toBe(152);
  });

  it("the window is the island plus its shadow, plus room below when asked", () => {
    expect(windowSize(islandSize("capsule"))).toEqual({ width: 92 + 2 * SHADOW_PAD, height: 28 + 2 * SHADOW_PAD });
    expect(windowSize(islandSize("line"), 60).height).toBe(36 + 2 * SHADOW_PAD + 60);
  });
});

describe("dot", () => {
  it("speaks the language: blue is Juno listening, green is your words, red is failure", () => {
    expect(dotFor(UI.BAR_STATES_LISTENING).color).toBe("#0A84FF");
    expect(dotFor(UI.BAR_STATES_DICTATING).color).toBe("#30D158");
    expect(dotFor(UI.BAR_STATES_ERROR).color).toBe("#FF453A");
    expect(dotFor(UI.BAR_STATES_ERROR).motion).toBe("shake");
    expect(dotFor(UI.BAR_STATES_LOADING).motion).toBe("orbit");
    expect(dotFor(UI.BAR_STATES_DEFAULT).motion).toBe("slow");
  });

  it("has a look for every state", () => {
    for (const state of EVERY_STATE) {
      const d = dotFor(state);
      expect(d.color).toMatch(/^#/);
      expect(d.opacity).toBeGreaterThan(0);
    }
  });
});

describe("words", () => {
  const base = {
    transcriptionText: "",
    spokenText: "",
    currentError: null,
    question: "",
  };

  it("shows nothing while resting and typing", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_DEFAULT })).toBeNull();
    expect(wordsFor({ ...base, state: UI.BAR_STATES_INPUT })).toBeNull();
  });

  it("shows your words as you speak, and a quiet label before any arrive", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_LISTENING })).toEqual({ text: "Listening", tone: "dim" });
    expect(
      wordsFor({ ...base, state: UI.BAR_STATES_LISTENING, transcriptionText: "Send the draft" }),
    ).toEqual({ text: "Send the draft", tone: "live" });
    expect(
      wordsFor({
        ...base,
        state: UI.BAR_STATES_DICTATING,
        transcriptionText: "Send the",
        transcriptionProvisional: true,
      }),
    ).toEqual({ text: "Send the", tone: "provisional" });
  });

  it("keeps what you asked in view while Juno works, and names a running tool", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_SUBMITTING, question: "Send it" })).toEqual({
      text: "Send it",
      tone: "dim",
    });
    expect(
      wordsFor({ ...base, state: UI.BAR_STATES_LOADING, question: "Send it", runningTool: "Reading the page" }),
    ).toEqual({ text: "Reading the page", tone: "plain" });
    expect(wordsFor({ ...base, state: UI.BAR_STATES_LOADING })).toEqual({ text: "Working", tone: "dim" });
  });

  it("says what went wrong, in red, and stays in sentence case", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_ERROR, currentError: "Connection unavailable" })).toEqual({
      text: "Connection unavailable",
      tone: "error",
    });
    expect(wordsFor({ ...base, state: UI.BAR_STATES_STOPPING })?.text).toBe("Stopping");
    expect(wordsFor({ ...base, state: UI.BAR_STATES_SUCCESS })?.text).toBe("Done");
  });

  it("the driving label wins over everything", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_LISTENING, drivingLabel: "using the mouse" })).toEqual({
      text: "using the mouse",
      tone: "plain",
    });
  });
});

describe("ring", () => {
  it("counts only once the answer is complete, Juno is idle and nothing waits on you", () => {
    expect(ringShouldRun({ cardOpen: true, streaming: false, working: false, approvalPending: false })).toBe(true);
    expect(ringShouldRun({ cardOpen: true, streaming: true, working: false, approvalPending: false })).toBe(false);
    expect(ringShouldRun({ cardOpen: true, streaming: false, working: true, approvalPending: false })).toBe(false);
    expect(ringShouldRun({ cardOpen: true, streaming: false, working: false, approvalPending: true })).toBe(false);
    expect(ringShouldRun({ cardOpen: false, streaming: false, working: false, approvalPending: false })).toBe(false);
  });
});

describe("latestTurn", () => {
  const user = (content: string): ChatMessage => ({ role: "user", content, timestamp: 1 });
  const assistant = (content: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
    role: "assistant",
    content,
    timestamp: 2,
    ...extra,
  });
  const tool = (content: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
    role: "tool_call_request",
    content,
    tool_name: "browser",
    tool_id: "t1",
    ...extra,
  });

  it("is the last question and everything since it", () => {
    const turn = latestTurn([
      user("First"),
      assistant("Old answer", { messageId: "m1" }),
      user("Second"),
      assistant("New answer", { messageId: "m2" }),
    ]);
    expect(turn.question).toBe("Second");
    expect(turn.answer?.messageId).toBe("m2");
    expect(turn.approval).toBeNull();
    expect(turn.runningTool).toBeNull();
  });

  it("finds a tool waiting on you, and a tool still running", () => {
    expect(latestTurn([user("Go"), tool("open the page", { approval_state: "pending" })]).approval?.content).toBe(
      "open the page",
    );
    expect(latestTurn([user("Go"), tool("Reading the page")]).runningTool).toBe("Reading the page");
    expect(latestTurn([user("Go"), tool("Reading the page", { success: true })]).runningTool).toBeNull();
    expect(latestTurn([user("Go"), tool("Reading", { approval_state: "denied" })]).runningTool).toBeNull();
  });

  it("counts a spoken-only reply as an answer, and ignores notices", () => {
    const spoken = assistant("", {
      messageId: "s",
      tts_metadata: { has_spoken_content: true, tts_parts: ["Done."], total_spoken_text: "Done." },
    });
    expect(latestTurn([user("Go"), spoken]).answer?.messageId).toBe("s");
    expect(latestTurn([user("Go"), assistant("Welcome to Juno", { notice: true })]).answer).toBeNull();
  });

  it("keys an answer by its message id so growth is not a new answer", () => {
    const a = latestTurn([user("Go"), assistant("Hel", { messageId: "m" })]);
    const b = latestTurn([user("Go"), assistant("Hello", { messageId: "m" })]);
    expect(answerKey(a)).toBe(answerKey(b));
    expect(answerKey(latestTurn([]))).toBe("");
  });
});
