import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  COMPOSER_LINE_PX,
  COMPOSER_MAX_TEXT_PX,
  HOVER_SHAPE,
  IDLE_SHAPE,
  ISLAND_STEADY_SPEC,
  ISLAND_WIDTH,
  SHADOW_PAD,
  SHAPE_MAX_HEIGHT,
  STOP_LABELS,
  answerKey,
  composerHeightFor,
  composerPlaceholder,
  dotFor,
  faceFor,
  latestTurn,
  lingerShouldRun,
  shapeFor,
  stopControl,
  wordsFor,
  type FaceInput,
} from "../islandModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

const at = (state: string, extra: Partial<FaceInput> = {}) =>
  faceFor({ state, answerOpen: false, componentShown: false, composerActive: false, dictation: false, ...extra });

const RESTING: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

describe("face", () => {
  it("rests as the capsule, and past rest the island is the text box in every state", () => {
    for (const state of EVERY_STATE) {
      expect(at(state)).toBe(RESTING.includes(state) ? "idle" : "compose");
    }
  });

  it("a draft, the caret or a step back keeps the composer up at rest", () => {
    expect(at(UI.BAR_STATES_DEFAULT, { composerActive: true })).toBe("compose");
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING, { composerActive: true })).toBe("compose");
  });

  it("an open answer shows, as a component when it is one", () => {
    expect(at(UI.BAR_STATES_DEFAULT, { answerOpen: true })).toBe("answer");
    expect(at(UI.BAR_STATES_AGENT_RESPONDING, { answerOpen: true })).toBe("answer");
    expect(at(UI.BAR_STATES_INPUT, { answerOpen: true })).toBe("answer");
    expect(at(UI.BAR_STATES_DEFAULT, { answerOpen: true, componentShown: true })).toBe("component");
  });

  it("a spoken turn to Juno turns the answer back into the composer", () => {
    expect(at(UI.BAR_STATES_LISTENING, { answerOpen: true })).toBe("compose");
    expect(at(UI.BAR_STATES_LISTENING, { answerOpen: true, componentShown: true })).toBe("compose");
    // The wake phrase merely armed is rest, not a turn.
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING, { answerOpen: true })).toBe("answer");
  });

  it("a dictation leaves the answer up, so the follow-up field it types into stays", () => {
    expect(at(UI.BAR_STATES_DICTATING, { answerOpen: true, dictation: true })).toBe("answer");
    expect(at(UI.BAR_STATES_LISTENING, { answerOpen: true, dictation: true })).toBe("answer");
    expect(at(UI.BAR_STATES_TRANSCRIBING, { answerOpen: true, dictation: true })).toBe("answer");
  });
});

describe("shape", () => {
  const base = { hovered: false, composerHeight: composerHeightFor(COMPOSER_LINE_PX), contentHeight: 0 };

  it("the capsule is a mark, and hover barely grows it", () => {
    expect(shapeFor("idle", base)).toEqual(IDLE_SHAPE);
    expect(shapeFor("idle", { ...base, hovered: true })).toEqual(HOVER_SHAPE);
    expect(IDLE_SHAPE.radius).toBe(IDLE_SHAPE.height / 2);
    expect(HOVER_SHAPE.height - IDLE_SHAPE.height).toBeLessThanOrEqual(4);
  });

  it("every face past idle is one width, so the composer never changes width", () => {
    for (const face of ["compose", "answer", "component"] as const) {
      expect(shapeFor(face, { ...base, contentHeight: 120 }).width).toBe(ISLAND_WIDTH);
    }
  });

  it("the composer grows with its text to four lines", () => {
    expect(composerHeightFor(0)).toBe(36);
    expect(composerHeightFor(COMPOSER_LINE_PX * 2)).toBe(54);
    expect(composerHeightFor(9000)).toBe(COMPOSER_MAX_TEXT_PX + 18);
    expect(shapeFor("compose", { ...base, composerHeight: 54 }).height).toBe(54);
  });

  it("an answer is its content plus the composer, up to the cap", () => {
    expect(shapeFor("answer", { ...base, contentHeight: 120 }).height).toBe(156);
    expect(shapeFor("answer", { ...base, contentHeight: 9000 }).height).toBe(SHAPE_MAX_HEIGHT);
  });

  it("the window holds the largest face once, and wells are the capsule's", () => {
    expect(ISLAND_STEADY_SPEC.rest).toEqual({
      width: IDLE_SHAPE.width + 2 * SHADOW_PAD,
      height: IDLE_SHAPE.height + 2 * SHADOW_PAD,
    });
    expect(ISLAND_STEADY_SPEC.max.width).toBe(ISLAND_WIDTH + 2 * SHADOW_PAD);
    expect(ISLAND_STEADY_SPEC.max.height).toBeGreaterThanOrEqual(SHAPE_MAX_HEIGHT + 2 * SHADOW_PAD);
  });
});

describe("placeholder", () => {
  const base = {
    transcriptionText: "",
    spokenText: "",
    currentError: null,
    working: false,
    answerShown: false,
  };
  const say = (state: string, extra: Partial<Parameters<typeof composerPlaceholder>[0]> = {}) =>
    composerPlaceholder({ ...base, state, ...extra });

  it("says what is happening, in sentence case", () => {
    expect(say(UI.BAR_STATES_DEFAULT).text).toBe("Ask Juno");
    expect(say(UI.BAR_STATES_INPUT, { answerShown: true }).text).toBe("Follow up");
    expect(say(UI.BAR_STATES_LISTENING).text).toBe("Listening…");
    expect(say(UI.BAR_STATES_TRANSCRIBING).text).toBe("Transcribing…");
    expect(say(UI.BAR_STATES_SUBMITTING).text).toBe("Sending…");
    expect(say(UI.BAR_STATES_LOADING).text).toBe("Working…");
    expect(say(UI.BAR_STATES_LOADING, { runningTool: "Reading the page" }).text).toBe("Reading the page…");
    expect(say(UI.BAR_STATES_STOPPING).text).toBe("Stopping…");
    expect(say(UI.BAR_STATES_ERROR, { currentError: "Connection unavailable" })).toEqual({
      text: "Connection unavailable",
      tone: "error",
    });
  });

  it("shows your words live while the mic is open", () => {
    expect(say(UI.BAR_STATES_LISTENING, { transcriptionText: "Send the draft" })).toEqual({
      text: "Send the draft",
      tone: "live",
    });
    expect(
      say(UI.BAR_STATES_TRANSCRIBING, { transcriptionText: "Send the", transcriptionProvisional: true }),
    ).toEqual({ text: "Send the", tone: "provisional" });
  });

  it("speaks the spoken sentence when there is no answer to read, and names the driving", () => {
    expect(say(UI.BAR_STATES_SPEAKING, { spokenText: "Here you go." }).text).toBe("Here you go.");
    expect(say(UI.BAR_STATES_SPEAKING, { spokenText: "Here you go.", answerShown: true }).text).toBe("Follow up");
    expect(say(UI.BAR_STATES_LOADING, { drivingLabel: "using the mouse" }).text).toBe("Juno is using the mouse");
  });
});

describe("stop control", () => {
  const stop = (extra: Partial<Parameters<typeof stopControl>[0]>) =>
    stopControl({ face: "compose", state: UI.BAR_STATES_INPUT, working: false, driving: false, ...extra });

  it("is named for what it stops", () => {
    expect(stop({ state: UI.BAR_STATES_LISTENING })).toBe("cancel");
    expect(stop({ state: UI.BAR_STATES_TRANSCRIBING, provisional: true })).toBe("cancel");
    expect(stop({ state: UI.BAR_STATES_LOADING, working: true })).toBe("stop");
    expect(stop({ driving: true })).toBe("stop");
    expect(stop({})).toBe("close");
    expect(STOP_LABELS.cancel).toBe("Cancel without sending");
    expect(STOP_LABELS.stop).toBe("Stop Juno");
  });

  it("leaves an answer with nothing running to its own close control", () => {
    expect(stop({ face: "answer", state: UI.BAR_STATES_DEFAULT })).toBeNull();
  });
});

describe("linger", () => {
  const run = (extra: Partial<Parameters<typeof lingerShouldRun>[0]>) =>
    lingerShouldRun({
      face: "answer",
      streaming: false,
      working: false,
      approvalPending: false,
      composerEngaged: false,
      ...extra,
    });

  it("counts on a finished answer or component only", () => {
    expect(run({})).toBe(true);
    expect(run({ face: "component" })).toBe(true);
    expect(run({ face: "compose" })).toBe(false);
    expect(run({ face: "idle" })).toBe(false);
  });

  it("never while it streams, Juno works, a tool waits, or the composer is in use", () => {
    expect(run({ streaming: true })).toBe(false);
    expect(run({ working: true })).toBe(false);
    expect(run({ approvalPending: true })).toBe(false);
    expect(run({ composerEngaged: true })).toBe(false);
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
