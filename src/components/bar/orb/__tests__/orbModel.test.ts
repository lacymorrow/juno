import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  APPROVAL_HEIGHT,
  BOTTOM_PAD,
  CAPTION_GAP,
  CAPTION_HEIGHT,
  CAPTION_WIDTH,
  ORB_SCALE,
  SHEET_MAX_HEIGHT,
  SHEET_MIN_HEIGHT,
  SIDE_PAD,
  STAGE,
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  SYSTEM_RED,
  TINT_REST,
  answerCaption,
  answerKey,
  captionFor,
  hasComponent,
  hasSheetContent,
  latestSentence,
  latestTurn,
  loopMaySleep,
  orbLook,
  orbTargets,
  plainText,
  posture,
  settled,
  sheetHeightFor,
  windowSize,
} from "../orbModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

const look = (state: string) => orbLook({ state });

describe("orbLook: what the orb does", () => {
  it("rests small, dim and breathing, with no spin so the loop can sleep", () => {
    for (const s of [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_SHRINKING]) {
      const l = look(s);
      expect(l.tint).toEqual(TINT_REST);
      expect(l.scale).toBe(ORB_SCALE.rest);
      expect(l.opacity).toBeLessThan(1);
      expect(l.motion).toBe("breathe");
      expect(l.spin).toBe(0);
      expect(loopMaySleep(l)).toBe(true);
    }
  });

  it("wakes, still grey, when you can type", () => {
    for (const s of [UI.BAR_STATES_EXPANDING, UI.BAR_STATES_INPUT]) {
      const l = look(s);
      expect(l.tint).toEqual(TINT_REST);
      expect(l.scale).toBe(ORB_SCALE.awake);
      expect(l.opacity).toBe(1);
      expect(loopMaySleep(l)).toBe(true);
    }
  });

  it("takes system blue and swells while it hears you", () => {
    const l = look(UI.BAR_STATES_LISTENING);
    expect(l.tint[0]).toBe(SYSTEM_BLUE);
    expect(l.motion).toBe("swell");
    expect(l.scale).toBe(ORB_SCALE.voice);
    expect(loopMaySleep(l)).toBe(false);
  });

  it("takes system green while your words become text", () => {
    expect(look(UI.BAR_STATES_DICTATING).tint[0]).toBe(SYSTEM_GREEN);
    expect(look(UI.BAR_STATES_DICTATING).motion).toBe("swell");
    expect(look(UI.BAR_STATES_TRANSCRIBING).tint[0]).toBe(SYSTEM_GREEN);
    expect(look(UI.BAR_STATES_DICTATION_READY).tint[0]).toBe(SYSTEM_GREEN);
    expect(look(UI.BAR_STATES_DICTATION_READY).motion).toBe("breathe");
  });

  it("listens for the wake word as a dim blue breath", () => {
    const l = look(UI.BAR_STATES_ALWAYS_LISTENING);
    expect(l.tint[0]).toBe(SYSTEM_BLUE);
    expect(l.opacity).toBeLessThan(0.7);
    expect(l.motion).toBe("breathe");
  });

  it("turns while Juno works, and quickens as steps complete", () => {
    for (const s of [UI.BAR_STATES_SUBMITTING, UI.BAR_STATES_LOADING, UI.BAR_STATES_AGENT_RESPONDING]) {
      const l = look(s);
      expect(l.motion).toBe("spin");
      expect(l.spin).toBeGreaterThan(0);
      expect(l.pulsePeriod).toBeGreaterThan(0);
      expect(loopMaySleep(l)).toBe(false);
    }
    const none = orbLook({ state: UI.BAR_STATES_LOADING, stepsDone: 0 });
    const two = orbLook({ state: UI.BAR_STATES_LOADING, stepsDone: 2 });
    const many = orbLook({ state: UI.BAR_STATES_LOADING, stepsDone: 40 });
    expect(two.spin).toBeGreaterThan(none.spin);
    expect(two.pulsePeriod).toBeLessThan(none.pulsePeriod);
    // Bounded: a long task does not become a blur.
    expect(many.spin).toBeLessThanOrEqual(2);
  });

  it("ripples with Juno's speech", () => {
    const l = look(UI.BAR_STATES_SPEAKING);
    expect(l.motion).toBe("speak");
    expect(l.tint[0]).toBe(SYSTEM_BLUE);
  });

  it("flinches red once on an error, blooms green when done, dims while stopping", () => {
    expect(look(UI.BAR_STATES_ERROR).tint[0]).toBe(SYSTEM_RED);
    expect(look(UI.BAR_STATES_ERROR).motion).toBe("flinch");
    expect(look(UI.BAR_STATES_FINISHING).motion).toBe("bloom");
    expect(look(UI.BAR_STATES_SUCCESS).tint[0]).toBe(SYSTEM_GREEN);
    expect(look(UI.BAR_STATES_STOPPING).opacity).toBeLessThan(1);
    expect(look(UI.BAR_STATES_STOPPING).tint).toEqual(TINT_REST);
  });

  it("holds still for an approval, whatever Rust says", () => {
    const l = orbLook({ state: UI.BAR_STATES_LOADING, approvalPending: true });
    expect(l.motion).toBe("still");
    expect(l.spin).toBe(0);
    expect(l.pulsePeriod).toBe(0);
    expect(loopMaySleep(l)).toBe(true);
  });

  it("turns steadily while Juno drives the cursor", () => {
    const l = orbLook({ state: UI.BAR_STATES_DEFAULT, driving: true });
    expect(l.motion).toBe("spin");
    expect(l.tint[0]).not.toBe(TINT_REST[0]);
  });

  it("maps every state Rust can send to a look", () => {
    for (const state of EVERY_STATE) {
      const l = look(state);
      expect(l.scale).toBeGreaterThan(0);
      expect(l.scale).toBeLessThanOrEqual(1);
      expect(["breathe", "still", "swell", "spin", "speak", "flinch", "bloom"]).toContain(l.motion);
    }
  });

  it("uses one accent plus green and red, never anything else", () => {
    const firsts = new Set(EVERY_STATE.map((s) => look(s).tint[0]));
    for (const c of firsts) {
      expect([SYSTEM_BLUE, "#0066D6", SYSTEM_GREEN, SYSTEM_RED, TINT_REST[0]]).toContain(c);
    }
  });
});

describe("orbTargets: what the audio does", () => {
  it("swells the orb with your voice, and only then", () => {
    const quiet = orbTargets(look(UI.BAR_STATES_LISTENING), 0);
    const loud = orbTargets(look(UI.BAR_STATES_LISTENING), 1);
    expect(loud.scale).toBeGreaterThan(quiet.scale);
    expect(loud.scale).toBeLessThanOrEqual(1);
    expect(loud.swell).toBe(1);
    const working = orbTargets(look(UI.BAR_STATES_LOADING), 1);
    expect(working.scale).toBe(ORB_SCALE.work);
    expect(working.swell).toBe(0);
  });

  it("churns with Juno's speech more than it swells", () => {
    const t = orbTargets(look(UI.BAR_STATES_SPEAKING), 1);
    expect(t.turbulence).toBe(1);
    expect(t.swell).toBeLessThan(0.5);
  });

  it("ignores a broken level", () => {
    expect(orbTargets(look(UI.BAR_STATES_LISTENING), Number.NaN).swell).toBe(0);
    expect(orbTargets(look(UI.BAR_STATES_LISTENING), 7).swell).toBe(1);
  });

  it("knows when it has settled", () => {
    const t = { scale: 0.5, swell: 0, turbulence: 0.12, opacity: 0.7 };
    expect(settled(t, t)).toBe(true);
    expect(settled({ ...t, scale: 0.51 }, t)).toBe(false);
    expect(settled({ ...t, opacity: 0.6 }, t)).toBe(false);
  });
});

const base = {
  transcriptionText: "",
  spokenText: "",
  currentError: null,
  question: "",
};

describe("captionFor: the one line under the orb", () => {
  it("says nothing at rest, and nothing while it waits for the wake word", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_DEFAULT })).toBeNull();
    expect(captionFor({ ...base, state: UI.BAR_STATES_ALWAYS_LISTENING })).toBeNull();
    expect(captionFor({ ...base, state: UI.BAR_STATES_DICTATION_READY })).toBeNull();
    expect(captionFor({ ...base, state: UI.BAR_STATES_STOPPING })).toBeNull();
  });

  it("shows your words as they arrive, provisional ones in their own tone", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_LISTENING })).toBeNull();
    expect(captionFor({ ...base, state: UI.BAR_STATES_LISTENING, transcriptionText: "Send it" })).toEqual({
      kind: "words",
      text: "Send it",
      tone: "live",
    });
    expect(
      captionFor({ ...base, state: UI.BAR_STATES_DICTATING, transcriptionText: "Send", transcriptionProvisional: true }),
    ).toEqual({ kind: "words", text: "Send", tone: "provisional" });
    expect(captionFor({ ...base, state: UI.BAR_STATES_TRANSCRIBING, transcriptionText: "Send it" })).toEqual({
      kind: "words",
      text: "Send it",
      tone: "dim",
    });
  });

  it("keeps your question, dimmed, while Juno works, and names a running tool", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_SUBMITTING, question: "Send it" })).toEqual({
      kind: "words",
      text: "Send it",
      tone: "dim",
    });
    expect(
      captionFor({ ...base, state: UI.BAR_STATES_LOADING, question: "Send it", runningTool: "Sending the draft." }),
    ).toEqual({ kind: "words", text: "Sending the draft", tone: "plain" });
  });

  it("captions the answer one spoken sentence at a time", () => {
    const answer = { spokenParts: ["Done.", "The draft is with Maya."], visibleText: "Done. The draft is with Maya.", streaming: true };
    expect(captionFor({ ...base, state: UI.BAR_STATES_AGENT_RESPONDING, answer })).toEqual({
      kind: "words",
      text: "The draft is with Maya.",
      tone: "live",
    });
  });

  it("falls back to the latest finished sentence when nothing is spoken", () => {
    const streaming = { spokenParts: [], visibleText: "Done. The draft is wi", streaming: true };
    expect(answerCaption(streaming)?.text).toBe("Done.");
    const done = { spokenParts: [], visibleText: "Done. The draft is with Maya", streaming: false };
    expect(answerCaption(done)?.text).toBe("The draft is with Maya");
    expect(answerCaption({ spokenParts: [], visibleText: "", streaming: true })).toBeNull();
  });

  it("shows the sentence Juno is saying while it speaks", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_SPEAKING, spokenText: "Here you go." })).toEqual({
      kind: "words",
      text: "Here you go.",
      tone: "live",
    });
  });

  it("keeps a finished answer's last line while it lingers, then lets it go", () => {
    const answer = { spokenParts: ["All set."], visibleText: "All set.", streaming: false };
    expect(captionFor({ ...base, state: UI.BAR_STATES_DEFAULT, answer, lingering: true })?.kind).toBe("words");
    expect(captionFor({ ...base, state: UI.BAR_STATES_DEFAULT, answer, lingering: false })).toBeNull();
    expect(captionFor({ ...base, state: UI.BAR_STATES_FINISHING, answer })?.kind).toBe("words");
  });

  it("becomes a composer when Rust is in its input state", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_INPUT })).toEqual({ kind: "composer" });
    expect(captionFor({ ...base, state: UI.BAR_STATES_EXPANDING })).toEqual({ kind: "composer" });
  });

  it("becomes the question when a tool waits on you, over everything else", () => {
    expect(
      captionFor({ ...base, state: UI.BAR_STATES_ERROR, currentError: "Nope", approval: "open the page in Safari." }),
    ).toEqual({ kind: "approval", text: "Juno wants to open the page in Safari" });
  });

  it("says what went wrong, in red, and says so plainly when Rust does not", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_ERROR, currentError: "Connection unavailable" })).toEqual({
      kind: "words",
      text: "Connection unavailable",
      tone: "error",
    });
    expect(captionFor({ ...base, state: UI.BAR_STATES_ERROR })?.kind).toBe("words");
  });

  it("says Juno is using the mouse while it drives", () => {
    expect(captionFor({ ...base, state: UI.BAR_STATES_LOADING, drivingLabel: "using the mouse in Safari" })).toEqual({
      kind: "words",
      text: "Juno is using the mouse in Safari",
      tone: "plain",
    });
  });

  it("has a caption or none for every state Rust can send", () => {
    for (const state of EVERY_STATE) {
      const c = captionFor({ ...base, state, transcriptionText: "x", question: "q" });
      expect(c === null || ["words", "composer", "approval"].includes(c.kind)).toBe(true);
    }
  });
});

describe("sentences", () => {
  it("strips components and marks so a caption is one plain line", () => {
    expect(plainText('Done.\n\n<TaskSummaryCard title="Sent" />\n\nI will **let you know**.')).toBe(
      "Done. I will let you know.",
    );
    expect(plainText("See [the doc](http://x) `now`")).toBe("See the doc now");
  });

  it("picks the latest finished sentence while streaming", () => {
    expect(latestSentence("One. Two! Thr", true)).toBe("Two!");
    expect(latestSentence("One. Two! Thr", false)).toBe("Thr");
    expect(latestSentence("", true)).toBe("");
    expect(latestSentence("No end yet", true)).toBe("");
  });
});

describe("the window", () => {
  it("is the orb's stage alone at rest, and never wider than the caption", () => {
    expect(windowSize("orb")).toEqual({ width: STAGE, height: STAGE });
    expect(windowSize("caption")).toEqual({
      width: CAPTION_WIDTH + 2 * SIDE_PAD,
      height: STAGE + CAPTION_GAP + CAPTION_HEIGHT + BOTTOM_PAD,
    });
    expect(windowSize("approval").height).toBe(STAGE + CAPTION_GAP + APPROVAL_HEIGHT + BOTTOM_PAD);
    expect(windowSize("sheet", 200).height).toBe(STAGE + CAPTION_GAP + 200 + BOTTOM_PAD);
  });

  it("keeps the orb's centre at the same place in every posture", () => {
    // Top-anchored: the stage is always the first STAGE px of the window, and
    // it is centred horizontally, so a centre-stable resize never moves it.
    for (const p of ["orb", "caption", "approval", "sheet"] as const) {
      expect(windowSize(p).height).toBeGreaterThanOrEqual(STAGE);
    }
  });

  it("steps the sheet's height and clamps it", () => {
    expect(sheetHeightFor(10)).toBe(SHEET_MIN_HEIGHT);
    expect(sheetHeightFor(150)).toBe(152);
    expect(sheetHeightFor(9000)).toBe(SHEET_MAX_HEIGHT);
  });

  it("chooses the posture from what is under the orb", () => {
    expect(posture(null, false)).toBe("orb");
    expect(posture({ kind: "words", text: "x", tone: "live" }, false)).toBe("caption");
    expect(posture({ kind: "composer" }, false)).toBe("caption");
    expect(posture({ kind: "approval", text: "x" }, false)).toBe("approval");
    expect(posture({ kind: "words", text: "x", tone: "live" }, true)).toBe("sheet");
  });
});

describe("the current turn", () => {
  const msgs: ChatMessage[] = [
    { role: "user", content: "Old", timestamp: 1 },
    { role: "assistant", content: "Old answer", messageId: "a0", timestamp: 2 },
    { role: "user", content: "Send it", timestamp: 3 },
    { role: "tool_call_request", content: "Sending the draft", tool_name: "mail", success: true },
    { role: "tool_call_request", content: "Asking for Friday", tool_name: "mail" },
    { role: "assistant", content: "", messageId: "a1", isStreaming: true, timestamp: 4 },
  ];

  it("finds the last question, the running tool and the steps done", () => {
    const t = latestTurn(msgs);
    expect(t.question).toBe("Send it");
    expect(t.runningTool).toBe("Asking for Friday");
    expect(t.stepsDone).toBe(1);
    expect(t.answer).toBeNull();
    expect(t.approval).toBeNull();
  });

  it("counts a spoken-only reply as an answer, and a pending tool as an approval", () => {
    const t = latestTurn([
      { role: "user", content: "Q", timestamp: 1 },
      { role: "tool_call_request", content: "open it", tool_id: "t1", approval_state: "pending" },
      {
        role: "assistant",
        content: "",
        messageId: "a2",
        timestamp: 2,
        tts_metadata: { has_spoken_content: true, tts_parts: ["Sure."], total_spoken_text: "Sure." },
      },
    ]);
    expect(t.approval?.tool_id).toBe("t1");
    expect(answerKey(t)).toBe("a2");
    expect(hasSheetContent(t.answer)).toBe(false);
  });

  it("knows when the answer carries a component", () => {
    expect(hasComponent({ role: "assistant", content: 'Here <WeatherCard city="x" />' })).toBe(true);
    expect(hasComponent({ role: "assistant", content: "Plain text, 3 < 4." })).toBe(false);
    expect(hasComponent({ role: "assistant", content: "x", isJsx: true })).toBe(true);
  });
});
