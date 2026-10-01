import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  APPROVAL_HEIGHT,
  CAPTION_HEIGHT,
  HUE_BASE,
  HUE_COOL,
  HUE_GREEN,
  ORB_SCALE,
  PANEL_GAP,
  SHEET_MIN_HEIGHT,
  STAGE,
  hasSheet,
  hueDelta,
  isImpulse,
  loopMaySleep,
  mustOpenSheet,
  orbLook,
  orbTargets,
  posture,
  settled,
  windowSize,
  wordsFor,
  type OrbMotion,
  type OrbTargets,
} from "../shaderOrbModel";

/**
 * Every row of the state table in docs/plans/orb-appearance.md, as a test.
 * The model is pure, so the whole appearance is checkable without a GPU.
 */

const EVERY_STATE: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_EXPANDING,
  UI.BAR_STATES_INPUT,
  UI.BAR_STATES_SHRINKING,
  UI.BAR_STATES_SUBMITTING,
  UI.BAR_STATES_LOADING,
  UI.BAR_STATES_SUCCESS,
  UI.BAR_STATES_ERROR,
  UI.BAR_STATES_SPEAKING,
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_TRANSCRIBING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_ALWAYS_LISTENING,
  UI.BAR_STATES_FINISHING,
  UI.BAR_STATES_AGENT_RESPONDING,
  UI.BAR_STATES_STOPPING,
];

describe("the sphere, state by state", () => {
  it("has a deliberate expression for every state Rust can send", () => {
    const seen = new Map<string, OrbMotion>();
    for (const state of EVERY_STATE) {
      const look = orbLook({ state });
      expect(look.scale, state).toBeGreaterThan(0);
      expect(look.scale, state).toBeLessThanOrEqual(1);
      expect(look.bright, state).toBeGreaterThan(0);
      expect(look.bright, state).toBeLessThanOrEqual(1);
      expect(look.sat, state).toBeGreaterThan(0);
      expect(look.flow, state).toBeGreaterThanOrEqual(0);
      expect(look.spin, state).toBeGreaterThanOrEqual(0);
      seen.set(state, look.motion);
    }
    // Not one bucket for everything: the sphere really does several things.
    expect(new Set(seen.values()).size).toBeGreaterThanOrEqual(6);
  });

  it("rests small and quiet, and lets the loop sleep", () => {
    const look = orbLook({ state: UI.BAR_STATES_DEFAULT });
    const awake = orbLook({ state: UI.BAR_STATES_LISTENING });
    expect(look.motion).toBe("ember");
    expect(look.hue).toBe(HUE_BASE);
    expect(look.scale).toBe(ORB_SCALE.rest);
    expect(look.bright).toBeLessThan(awake.bright);
    expect(look.sat).toBeLessThan(awake.sat);
    expect(loopMaySleep(look)).toBe(true);
  });

  it("stays bright enough at rest to be found on a black wallpaper", () => {
    // The Orb has no chrome, so a dim ring on a dark desktop is a lost ring.
    // This is the floor that a tuning pass must not quietly drop below.
    for (const state of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_SHRINKING,
      UI.BAR_STATES_DICTATION_READY,
      UI.BAR_STATES_ALWAYS_LISTENING,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(orbLook({ state }).bright, state).toBeGreaterThanOrEqual(0.66);
    }
  });

  it("cools to blue and grows when the microphone is open for Juno", () => {
    const look = orbLook({ state: UI.BAR_STATES_LISTENING });
    expect(look.motion).toBe("hear");
    expect(look.hue).toBe(HUE_COOL);
    expect(look.scale).toBe(ORB_SCALE.voice);
    expect(loopMaySleep(look)).toBe(false);
  });

  it("goes green for dictation, because those words are yours", () => {
    expect(orbLook({ state: UI.BAR_STATES_DICTATING }).hue).toBe(HUE_GREEN);
    expect(orbLook({ state: UI.BAR_STATES_DICTATION_READY }).hue).toBe(HUE_GREEN);
    expect(orbLook({ state: UI.BAR_STATES_TRANSCRIBING }).hue).toBe(HUE_GREEN);
  });

  it("dims below rest while it waits for the wake word", () => {
    const wake = orbLook({ state: UI.BAR_STATES_ALWAYS_LISTENING });
    const rest = orbLook({ state: UI.BAR_STATES_DEFAULT });
    expect(wake.hue).toBe(HUE_COOL);
    expect(wake.bright).toBeLessThan(rest.bright);
  });

  it("keeps its own colours while it works: thinking is motion, not a hue", () => {
    const look = orbLook({ state: UI.BAR_STATES_LOADING });
    expect(look.motion).toBe("turn");
    expect(look.hue).toBe(HUE_BASE);
    expect(look.spin).toBeGreaterThan(0);
    expect(look.pulsePeriod).toBeGreaterThan(0);
  });

  it("quickens with every finished step, and stops counting after four", () => {
    const at = (steps: number) => orbLook({ state: UI.BAR_STATES_LOADING, stepsDone: steps });
    const periods = [0, 1, 2, 3, 4].map((n) => at(n).pulsePeriod);
    for (let i = 1; i < periods.length; i += 1) expect(periods[i]).toBeLessThan(periods[i - 1]);
    expect(at(9).pulsePeriod).toBe(at(4).pulsePeriod);
    expect(at(9).flow).toBe(at(4).flow);
    // A negative or fractional count from a half-parsed turn cannot slow it.
    expect(at(-3).pulsePeriod).toBe(at(0).pulsePeriod);
    expect(at(1.7).pulsePeriod).toBe(at(1).pulsePeriod);
  });

  it("stops dead when a tool is waiting on you", () => {
    const look = orbLook({ state: UI.BAR_STATES_LOADING, approvalPending: true });
    expect(look.motion).toBe("hold");
    expect(look.flow).toBe(0);
    expect(look.spin).toBe(0);
    expect(look.ripple).toBe(0);
    expect(look.pulsePeriod).toBe(0);
    // Frozen is a resting look as far as the loop is concerned.
    expect(loopMaySleep(look)).toBe(true);
  });

  it("holds still for an approval even in a state that otherwise swells", () => {
    expect(orbLook({ state: UI.BAR_STATES_LISTENING, approvalPending: true }).motion).toBe("hold");
    expect(orbLook({ state: UI.BAR_STATES_ERROR, approvalPending: true }).motion).toBe("hold");
  });

  it("collapses to one red object when something failed, and nowhere else", () => {
    const bad = orbLook({ state: UI.BAR_STATES_ERROR });
    expect(bad.motion).toBe("recoil");
    expect(bad.mono).toBe(1);
    for (const state of EVERY_STATE.filter((s) => s !== UI.BAR_STATES_ERROR)) {
      expect(orbLook({ state }).mono, state).toBe(0);
    }
  });

  it("blooms green once when the turn lands", () => {
    for (const state of [UI.BAR_STATES_FINISHING, UI.BAR_STATES_SUCCESS]) {
      const look = orbLook({ state });
      expect(look.motion, state).toBe("bloom");
      expect(look.hue, state).toBe(HUE_GREEN);
      expect(look.scale, state).toBe(ORB_SCALE.bloom);
    }
  });

  it("only treats the one-shot motions as one-shot", () => {
    expect(isImpulse("recoil")).toBe(true);
    expect(isImpulse("bloom")).toBe(true);
    for (const m of ["ember", "wake", "hear", "turn", "voice", "hold"] as OrbMotion[]) {
      expect(isImpulse(m), m).toBe(false);
    }
  });

  it("holds a green ember while an answer is unread, and only at rest", () => {
    const held = orbLook({ state: UI.BAR_STATES_DEFAULT, answerWaiting: true });
    const rest = orbLook({ state: UI.BAR_STATES_DEFAULT });
    expect(held.motion).toBe("ember");
    expect(held.hue).toBe(HUE_GREEN);
    expect(held.bright).toBeGreaterThan(rest.bright);
    expect(held.scale).toBeGreaterThan(rest.scale);
    // Mid-turn it is irrelevant: the work reads first.
    expect(orbLook({ state: UI.BAR_STATES_LOADING, answerWaiting: true }).motion).toBe("turn");
  });

  it("turns steadily with no pulse while Juno drives the cursor", () => {
    const look = orbLook({ state: UI.BAR_STATES_DEFAULT, driving: true });
    expect(look.motion).toBe("turn");
    expect(look.pulsePeriod).toBe(0);
    expect(look.spin).toBeGreaterThan(0);
  });

  it("falls back to rest for a state it has never heard of", () => {
    expect(orbLook({ state: "teleporting" }).motion).toBe("ember");
  });
});

describe("what the audio does", () => {
  it("swells and ripples with your voice", () => {
    const look = orbLook({ state: UI.BAR_STATES_LISTENING });
    const quiet = orbTargets(look, 0);
    const loud = orbTargets(look, 1);
    expect(loud.scale).toBeGreaterThan(quiet.scale);
    expect(loud.ripple).toBeGreaterThan(quiet.ripple);
    expect(loud.flow).toBeGreaterThan(quiet.flow);
  });

  it("ripples with Juno's speech more than it grows", () => {
    const look = orbLook({ state: UI.BAR_STATES_SPEAKING });
    const quiet = orbTargets(look, 0);
    const loud = orbTargets(look, 1);
    expect(loud.ripple - quiet.ripple).toBeGreaterThan(loud.scale - quiet.scale);
  });

  it("ignores the level in every look that is not a voice", () => {
    for (const state of [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_LOADING, UI.BAR_STATES_ERROR]) {
      const look = orbLook({ state });
      expect(orbTargets(look, 1), state).toEqual(orbTargets(look, 0));
    }
  });

  it("clamps a level that is out of range, missing or not a number", () => {
    const look = orbLook({ state: UI.BAR_STATES_LISTENING });
    expect(orbTargets(look, 4)).toEqual(orbTargets(look, 1));
    expect(orbTargets(look, -2)).toEqual(orbTargets(look, 0));
    expect(orbTargets(look, Number.NaN)).toEqual(orbTargets(look, 0));
  });
});

describe("the sleep rule", () => {
  const target = (over: Partial<OrbTargets> = {}): OrbTargets => ({
    hue: 0,
    sat: 0.35,
    mono: 0,
    bright: 0.55,
    scale: 0.52,
    ripple: 0,
    flow: 0.08,
    spin: 0,
    ...over,
  });

  it("settles when every value has arrived", () => {
    expect(settled(target(), target())).toBe(true);
  });

  it("does not settle while any one of them is still moving", () => {
    expect(settled(target({ scale: 0.6 }), target())).toBe(false);
    expect(settled(target({ hue: 90 }), target())).toBe(false);
    expect(settled(target({ mono: 0.5 }), target())).toBe(false);
    expect(settled(target({ bright: 1 }), target())).toBe(false);
    expect(settled(target({ ripple: 0.4 }), target())).toBe(false);
  });

  it("takes the short way round the hue circle", () => {
    expect(hueDelta(0, 350)).toBe(-10);
    expect(hueDelta(350, 0)).toBe(10);
    expect(hueDelta(0, 110)).toBe(110);
    expect(Math.abs(hueDelta(0, 180))).toBe(180);
  });

  it("never sleeps in a look that is meant to be moving", () => {
    for (const state of [
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_FINISHING,
    ]) {
      expect(loopMaySleep(orbLook({ state })), state).toBe(false);
    }
  });
});

describe("the words, which are the exception", () => {
  const base = { state: UI.BAR_STATES_DEFAULT, currentError: null };

  it("says nothing at all in the states that carry a status elsewhere", () => {
    for (const state of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_DICTATING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_STOPPING,
      UI.BAR_STATES_ALWAYS_LISTENING,
      UI.BAR_STATES_DICTATION_READY,
    ]) {
      expect(wordsFor({ ...base, state }), state).toBeNull();
    }
  });

  it("asks the question when a tool needs an answer, whatever the state", () => {
    const words = wordsFor({ ...base, state: UI.BAR_STATES_LOADING, approval: "open Safari." });
    expect(words).toEqual({ kind: "approval", text: "Juno wants to open Safari" });
  });

  it("shows the error itself, and a fallback when Rust sent none", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_ERROR, currentError: "No microphone" })).toEqual({
      kind: "error",
      text: "No microphone",
    });
    expect(wordsFor({ ...base, state: UI.BAR_STATES_ERROR })).toEqual({
      kind: "error",
      text: "Something went wrong",
    });
  });

  it("opens the composer when Rust says you are typing", () => {
    expect(wordsFor({ ...base, state: UI.BAR_STATES_INPUT })).toEqual({ kind: "composer" });
    expect(wordsFor({ ...base, state: UI.BAR_STATES_EXPANDING })).toEqual({ kind: "composer" });
  });

  it("lets an approval outrank an error and the composer", () => {
    const words = wordsFor({
      state: UI.BAR_STATES_ERROR,
      currentError: "boom",
      approval: "delete a file",
    });
    expect(words?.kind).toBe("approval");
  });
});

describe("the window", () => {
  it("is the sphere and nothing else when no words are needed", () => {
    expect(posture(null, false)).toBe("orb");
    expect(windowSize("orb")).toEqual({ width: STAGE, height: STAGE });
  });

  it("puts an approval above everything, the sheet you opened next", () => {
    expect(posture({ kind: "approval", text: "x" }, true)).toBe("approval");
    expect(posture({ kind: "composer" }, true)).toBe("sheet");
    expect(posture({ kind: "error", text: "x" }, false)).toBe("words");
  });

  it("keeps the sphere's centre in the same place in every posture", () => {
    const centre = (p: Parameters<typeof windowSize>[0]) => ({
      x: windowSize(p).width / 2,
      y: STAGE / 2,
    });
    const orb = centre("orb");
    expect(centre("words").y).toBe(orb.y);
    expect(centre("approval").y).toBe(orb.y);
    expect(centre("sheet").y).toBe(orb.y);
  });

  it("gives the panel postures one width and a height that fits their content", () => {
    const words = windowSize("words");
    const approval = windowSize("approval");
    const sheet = windowSize("sheet", 200);
    expect(approval.width).toBe(words.width);
    expect(sheet.width).toBe(words.width);
    expect(words.height).toBe(STAGE + PANEL_GAP + CAPTION_HEIGHT + 16);
    expect(approval.height).toBe(STAGE + PANEL_GAP + APPROVAL_HEIGHT + 16);
    expect(sheet.height).toBeGreaterThan(approval.height);
  });

  it("never lets a measured sheet push the window past its range", () => {
    const tiny = windowSize("sheet", 1);
    const huge = windowSize("sheet", 10_000);
    expect(tiny.height).toBe(STAGE + PANEL_GAP + SHEET_MIN_HEIGHT + 16);
    expect(huge.height).toBeLessThan(STAGE + PANEL_GAP + 400);
  });
});

describe("what the sheet is for", () => {
  const msg = (over: Partial<ChatMessage> = {}): ChatMessage =>
    ({ role: "assistant", content: "", timestamp: 1, ...over }) as ChatMessage;

  it("is worth opening when there is an answer, or only the question", () => {
    expect(hasSheet({ question: "", answer: null, streaming: false })).toBe(false);
    expect(hasSheet({ question: "what time is it", answer: null, streaming: false })).toBe(true);
    expect(hasSheet({ question: "", answer: msg({ content: "3pm" }), streaming: false })).toBe(true);
  });

  it("is worth opening for a request for the cursor with nothing else to show", () => {
    expect(hasSheet({ question: "", answer: null, streaming: false }, true)).toBe(true);
  });

  it("opens by itself only for an answer that cannot be spoken", () => {
    expect(mustOpenSheet(null)).toBe(false);
    expect(mustOpenSheet(msg({ content: "just words" }))).toBe(false);
    expect(mustOpenSheet(msg({ content: "<NowPlayingCard />" }))).toBe(true);
    expect(mustOpenSheet(msg({ content: "spoken", isJsx: true }))).toBe(true);
    // A spoken answer's own marker is not a component.
    expect(mustOpenSheet(msg({ content: "<TTS>hello</TTS>" }))).toBe(false);
  });
});
