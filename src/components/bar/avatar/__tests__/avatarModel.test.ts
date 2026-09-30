import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import {
  HEAD,
  HEAD_ANCHOR,
  PAD,
  PANEL_WIDTH,
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  headFor,
  junoBubbleFor,
  lingerShouldRun,
  sceneFor,
  stackHeightFor,
  windowFor,
  yourBubbleFor,
  type SceneInput,
} from "../avatarModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

const at = (state: string, extra: Partial<SceneInput> = {}): SceneInput => ({
  state,
  transcriptionText: "",
  spokenText: "",
  currentError: null,
  question: "",
  answerOpen: false,
  hasAnswerContent: false,
  approvalPending: false,
  ...extra,
});

describe("the head", () => {
  it("rests calm with no cue and nothing in Rive", () => {
    expect(headFor(at(UI.BAR_STATES_DEFAULT))).toEqual({ rive: "idle", gesture: "calm", cue: null });
    expect(headFor(at(UI.BAR_STATES_SHRINKING))).toEqual({ rive: "idle", gesture: "calm", cue: null });
  });

  it("leans in with a blue cue while listening, green while your words become text", () => {
    const listening = headFor(at(UI.BAR_STATES_LISTENING));
    expect(listening.rive).toBe("listening");
    expect(listening.gesture).toBe("lean");
    expect(listening.cue?.color).toBe(SYSTEM_BLUE);
    expect(listening.cue?.motion).toBe("breathe");

    const dictating = headFor(at(UI.BAR_STATES_DICTATING));
    expect(dictating.gesture).toBe("lean");
    expect(dictating.cue?.color).toBe(SYSTEM_GREEN);

    const ready = headFor(at(UI.BAR_STATES_DICTATION_READY));
    expect(ready.gesture).toBe("calm");
    expect(ready.cue).toEqual({ color: SYSTEM_GREEN, opacity: 0.6, motion: "still" });

    const always = headFor(at(UI.BAR_STATES_ALWAYS_LISTENING));
    expect(always.gesture).toBe("calm");
    expect(always.rive).toBe("listening");
    expect(always.cue?.motion).toBe("slow");
  });

  it("attends the composer without leaning", () => {
    expect(headFor(at(UI.BAR_STATES_INPUT))).toEqual({ rive: "idle", gesture: "attend", cue: null });
    expect(headFor(at(UI.BAR_STATES_EXPANDING)).gesture).toBe("attend");
  });

  it("thinks while Rust works, talks once the answer has content", () => {
    for (const s of [
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_STOPPING,
    ]) {
      const h = headFor(at(s));
      expect(h.rive).toBe("thinking");
      expect(h.gesture).toBe("think");
    }
    expect(headFor(at(UI.BAR_STATES_AGENT_RESPONDING))).toMatchObject({ rive: "thinking", gesture: "think" });
    expect(headFor(at(UI.BAR_STATES_AGENT_RESPONDING, { hasAnswerContent: true }))).toMatchObject({
      rive: "speaking",
      gesture: "talk",
    });
    expect(headFor(at(UI.BAR_STATES_SPEAKING))).toMatchObject({ rive: "speaking", gesture: "talk" });
  });

  it("nods when finished, winces on an error, goes wide-eyed when a tool needs you", () => {
    expect(headFor(at(UI.BAR_STATES_FINISHING)).gesture).toBe("nod");
    expect(headFor(at(UI.BAR_STATES_SUCCESS)).gesture).toBe("nod");
    expect(headFor(at(UI.BAR_STATES_ERROR)).gesture).toBe("wince");
    expect(headFor(at(UI.BAR_STATES_LOADING, { approvalPending: true })).gesture).toBe("wide");
    // A failure wins over a pending question: the person must see it.
    expect(headFor(at(UI.BAR_STATES_ERROR, { approvalPending: true })).gesture).toBe("wince");
  });

  it("thinks while driving the cursor, whatever Rust says", () => {
    expect(headFor(at(UI.BAR_STATES_INPUT, { drivingLabel: "using the mouse" }))).toMatchObject({
      rive: "thinking",
      gesture: "think",
    });
  });

  it("gives every state Rust can send a rive input and a gesture", () => {
    for (const state of EVERY_STATE) {
      const h = headFor(at(state));
      expect(["idle", "listening", "thinking", "speaking"]).toContain(h.rive);
      expect(["calm", "lean", "attend", "think", "talk", "nod", "wince", "wide"]).toContain(h.gesture);
    }
  });
});

describe("your bubble", () => {
  it("is the composer while Rust wants typing, disabled until the window has opened", () => {
    expect(yourBubbleFor(at(UI.BAR_STATES_EXPANDING))).toEqual({ kind: "composer", ready: false });
    expect(yourBubbleFor(at(UI.BAR_STATES_INPUT))).toEqual({ kind: "composer", ready: true });
  });

  it("shows your words as you speak, a placeholder before the first one", () => {
    expect(yourBubbleFor(at(UI.BAR_STATES_LISTENING))).toEqual({ kind: "words", text: "Listening", tone: "dim" });
    expect(yourBubbleFor(at(UI.BAR_STATES_LISTENING, { transcriptionText: "Send it" }))).toEqual({
      kind: "words",
      text: "Send it",
      tone: "live",
    });
    expect(
      yourBubbleFor(at(UI.BAR_STATES_LISTENING, { transcriptionText: "Send", transcriptionProvisional: true })),
    ).toEqual({ kind: "words", text: "Send", tone: "provisional" });
  });

  it("tints dictation green and keeps the final transcript dim while it settles", () => {
    expect(yourBubbleFor(at(UI.BAR_STATES_DICTATING, { transcriptionText: "Dear Maya" }))).toEqual({
      kind: "words",
      text: "Dear Maya",
      tone: "dictation",
    });
    expect(yourBubbleFor(at(UI.BAR_STATES_DICTATING))).toEqual({ kind: "words", text: "Dictating", tone: "dim" });
    expect(yourBubbleFor(at(UI.BAR_STATES_TRANSCRIBING, { transcriptionText: "Dear Maya" }))).toEqual({
      kind: "words",
      text: "Dear Maya",
      tone: "dim",
    });
    expect(yourBubbleFor(at(UI.BAR_STATES_ALWAYS_LISTENING))?.kind).toBe("words");
  });

  it("keeps what you asked, dimmed, while Juno works and answers", () => {
    for (const s of [
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(yourBubbleFor(at(s, { question: "What time is it" }))).toEqual({
        kind: "words",
        text: "What time is it",
        tone: "dim",
      });
      expect(yourBubbleFor(at(s))).toBeNull();
    }
  });

  it("stays above an answer that is still up, and is gone at rest otherwise", () => {
    expect(yourBubbleFor(at(UI.BAR_STATES_DEFAULT, { question: "Q", answerOpen: true }))).toEqual({
      kind: "words",
      text: "Q",
      tone: "dim",
    });
    expect(yourBubbleFor(at(UI.BAR_STATES_DEFAULT, { question: "Q" }))).toBeNull();
    expect(yourBubbleFor(at(UI.BAR_STATES_DICTATION_READY))).toBeNull();
    expect(yourBubbleFor(at(UI.BAR_STATES_SHRINKING))).toBeNull();
  });
});

describe("Juno's bubble", () => {
  it("is a thought while working, with the running tool named inside", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_SUBMITTING))).toEqual({ kind: "thought", text: "Thinking" });
    expect(junoBubbleFor(at(UI.BAR_STATES_LOADING))).toEqual({ kind: "thought", text: "Working" });
    expect(junoBubbleFor(at(UI.BAR_STATES_LOADING, { runningTool: "Reading the page" }))).toEqual({
      kind: "thought",
      text: "Reading the page",
    });
    expect(junoBubbleFor(at(UI.BAR_STATES_AGENT_RESPONDING))).toEqual({ kind: "thought", text: "Working" });
    expect(junoBubbleFor(at(UI.BAR_STATES_STOPPING))).toEqual({ kind: "thought", text: "Stopping" });
  });

  it("is the answer once one is on stage with content", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_AGENT_RESPONDING, { answerOpen: true, hasAnswerContent: true }))).toEqual({
      kind: "answer",
    });
    expect(junoBubbleFor(at(UI.BAR_STATES_DEFAULT, { answerOpen: true, hasAnswerContent: true }))).toEqual({
      kind: "answer",
    });
    // Open but empty (the first chunk has not landed): still a thought.
    expect(junoBubbleFor(at(UI.BAR_STATES_AGENT_RESPONDING, { answerOpen: true }))).toEqual({
      kind: "thought",
      text: "Working",
    });
  });

  it("speaks the sentence when there is no answer to show", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_SPEAKING, { spokenText: "Done." }))).toEqual({
      kind: "speech",
      text: "Done.",
    });
    expect(junoBubbleFor(at(UI.BAR_STATES_SPEAKING))).toBeNull();
    expect(
      junoBubbleFor(at(UI.BAR_STATES_SPEAKING, { spokenText: "Done.", answerOpen: true, hasAnswerContent: true })),
    ).toEqual({ kind: "answer" });
  });

  it("holds up the question when a tool waits on you, over everything but a failure", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_LOADING, { approvalPending: true }))).toEqual({ kind: "question" });
    expect(
      junoBubbleFor(at(UI.BAR_STATES_DEFAULT, { approvalPending: true, answerOpen: true, hasAnswerContent: true })),
    ).toEqual({ kind: "question" });
  });

  it("shows a failure with its message, or a plain one", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_ERROR, { currentError: "Connection unavailable" }))).toEqual({
      kind: "error",
      text: "Connection unavailable",
    });
    expect(junoBubbleFor(at(UI.BAR_STATES_ERROR))).toEqual({ kind: "error", text: "Something went wrong" });
  });

  it("says so while Juno drives the cursor", () => {
    expect(junoBubbleFor(at(UI.BAR_STATES_LOADING, { drivingLabel: "using the mouse in Safari" }))).toEqual({
      kind: "thought",
      text: "Juno is using the mouse in Safari",
    });
  });

  it("is empty at rest and while you speak or type", () => {
    for (const s of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_SHRINKING,
      UI.BAR_STATES_DICTATION_READY,
      UI.BAR_STATES_EXPANDING,
      UI.BAR_STATES_INPUT,
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_DICTATING,
      UI.BAR_STATES_ALWAYS_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
    ]) {
      expect(junoBubbleFor(at(s))).toBeNull();
    }
  });
});

describe("the scene, every state", () => {
  it("maps every state Rust can send to a head and at most two bubbles", () => {
    for (const state of EVERY_STATE) {
      const scene = sceneFor(at(state, { question: "Q" }));
      expect(scene.head).toBeTruthy();
      expect([null, "words", "composer"]).toContain(scene.yours?.kind ?? null);
      expect([null, "thought", "speech", "answer", "question", "error"]).toContain(scene.junos?.kind ?? null);
    }
  });
});

describe("the window", () => {
  it("is the head alone at rest, and keeps the head's centre at the same anchor when open", () => {
    const rest = windowFor(sceneFor(at(UI.BAR_STATES_DEFAULT)), 0);
    expect(rest).toEqual({ width: HEAD + 2 * PAD, height: HEAD + 2 * PAD, anchorY: HEAD_ANCHOR });
    const open = windowFor(sceneFor(at(UI.BAR_STATES_LISTENING)), 40);
    expect(open.width).toBe(PANEL_WIDTH);
    expect(open.anchorY).toBe(rest.anchorY);
    expect(open.height).toBe(PAD + HEAD + 10 + 40 + PAD);
  });

  it("steps the stack height so a streaming answer does not resize on every glyph", () => {
    expect(stackHeightFor(0)).toBe(0);
    expect(stackHeightFor(1)).toBe(8);
    expect(stackHeightFor(40)).toBe(40);
    expect(stackHeightFor(41)).toBe(48);
    expect(windowFor(sceneFor(at(UI.BAR_STATES_LISTENING)), 41).height).toBe(PAD + HEAD + 10 + 48 + PAD);
  });

  it("has the same width for listening, thinking and answering, so the head never slides", () => {
    const a = windowFor(sceneFor(at(UI.BAR_STATES_LISTENING)), 40).width;
    const b = windowFor(sceneFor(at(UI.BAR_STATES_LOADING, { question: "Q" })), 80).width;
    const c = windowFor(sceneFor(at(UI.BAR_STATES_DEFAULT, { answerOpen: true, hasAnswerContent: true })), 200).width;
    expect(a).toBe(b);
    expect(b).toBe(c);
  });
});

describe("the linger", () => {
  it("counts only when an answer is up, finished, idle, unquestioned, and you are not typing", () => {
    const base = { answerOpen: true, streaming: false, working: false, approvalPending: false, composerOpen: false };
    expect(lingerShouldRun(base)).toBe(true);
    expect(lingerShouldRun({ ...base, answerOpen: false })).toBe(false);
    expect(lingerShouldRun({ ...base, streaming: true })).toBe(false);
    expect(lingerShouldRun({ ...base, working: true })).toBe(false);
    expect(lingerShouldRun({ ...base, approvalPending: true })).toBe(false);
    expect(lingerShouldRun({ ...base, composerOpen: true })).toBe(false);
  });
});
