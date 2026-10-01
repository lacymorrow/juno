import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  APPROVAL_HEIGHT,
  CAPTION_HEIGHT,
  CAPTION_WIDTH,
  SHEET_MIN_HEIGHT,
  sheetHeightFor,
} from "../orb/orbModel";

/**
 * The Orb's model: pure functions from what Rust says (the bar state, the
 * audio level) and what the orb knows locally (the turn, an approval, an
 * answer nobody has read yet) to what the sphere does and whether any words
 * are needed at all.
 *
 * Nothing here touches the DOM, WebGL or Tauri, so every row of the state
 * table in docs/plans/orb-appearance.md is a unit test.
 *
 * The words panel is Presence's, deliberately: its geometry and its pieces
 * are imported from ../orb rather than copied, so the two looks' panels are
 * the same object. Only the canvas and the policy below are the Orb's own.
 */

// === GEOMETRY ===

/** The canvas is this big, once. The sphere scales itself inside it. */
export const STAGE = 160;

/** Room beside the panel for its shadow. */
export const SIDE_PAD = 20;
/** Room under the panel. */
export const BOTTOM_PAD = 16;
/** Gap between the sphere's stage and the panel. */
export const PANEL_GAP = 6;

export { CAPTION_WIDTH, CAPTION_HEIGHT, APPROVAL_HEIGHT, SHEET_MIN_HEIGHT, sheetHeightFor };

/** How long an unread answer keeps the sphere's green ember before it rests. */
export const HELD_MS = 12_000;
/** A shrinking window waits for the panel's fade before it snaps. */
export const SHRINK_DELAY_MS = 350;
/** Rest runs on for this long after the last change, then the loop sleeps:
 *  the sphere comes to a stop instead of cutting to a still frame. */
export const REST_DRIFT_MS = 2_500;

// === COLOUR ===

/**
 * One knob rotates all three of the shader's base colours together, so the
 * sphere is always the same object rather than a repainted one. Three hues
 * earn a place, and each one is a message that must not be missed.
 */
/** Violet, cyan and a deep blue core: the orb's own colours. */
export const HUE_BASE = 0;
/** Violet to sky blue: the microphone is open for Juno. */
export const HUE_COOL = 350;
/** Teal to lime: your words landed, or Juno is done. */
export const HUE_GREEN = 110;

// === STATE GROUPS ===

const REST_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_SHRINKING,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

const INPUT_STATES: readonly string[] = [UI.BAR_STATES_INPUT, UI.BAR_STATES_EXPANDING];

const VOICE_STATES: readonly string[] = [
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

/** States in which Rust is busy on the person's behalf. Escape stops work. */
export const WORKING_STATES: readonly string[] = [
  UI.BAR_STATES_TRANSCRIBING,
  UI.BAR_STATES_SUBMITTING,
  UI.BAR_STATES_LOADING,
  UI.BAR_STATES_AGENT_RESPONDING,
  UI.BAR_STATES_FINISHING,
  UI.BAR_STATES_STOPPING,
];

export const isRestState = (s: string): boolean => REST_STATES.includes(s);
export const isInputState = (s: string): boolean => INPUT_STATES.includes(s);
export const isVoiceState = (s: string): boolean => VOICE_STATES.includes(s);
export const isWorkingState = (s: string): boolean => WORKING_STATES.includes(s);

// === THE SPHERE ===

/** How the sphere behaves, beyond what the audio does to it. */
export type OrbMotion =
  /** Small, dim, its inside drifting to a stop; a CSS breath carries it. */
  | "ember"
  /** Awake and waiting on you: brighter and larger, inside still slow. */
  | "wake"
  /** Swells and ripples with your voice. */
  | "hear"
  /** Turning, its inside racing; the pulse cadence is the progress. */
  | "turn"
  /** Ripples with Juno's speech. */
  | "voice"
  /** Stopped dead: Juno is waiting on your answer. */
  | "hold"
  /** One inward flinch, then it holds red. */
  | "recoil"
  /** One outward bloom, then it settles. */
  | "bloom";

export interface OrbLook {
  motion: OrbMotion;
  /** Degrees of rotation applied to all three base colours. */
  hue: number;
  /** Chroma: 0 is grey, 1 is the full base colours. */
  sat: number;
  /** Collapses the three colours onto one red object. Error only. */
  mono: number;
  /** How present the sphere is, 0 to 1. */
  bright: number;
  /** How much of the stage the sphere fills. */
  scale: number;
  /** Surface disturbance before the audio adds to it. */
  ripple: number;
  /** Speed of the inside: the noisy rim, the colour sweep, the moving light. */
  flow: number;
  /** Rigid rotation of the whole sphere, radians per second. */
  spin: number;
  /** Pulse period in seconds; 0 means no pulse. */
  pulsePeriod: number;
}

export interface OrbLookInput {
  state: string;
  /** Tool calls finished in this turn; the pulse quickens with each. */
  stepsDone?: number;
  /** A tool is waiting on Allow or Don't. */
  approvalPending?: boolean;
  /** Juno holds the physical cursor. */
  driving?: boolean;
  /** An answer is waiting to be read. Rest keeps a green ember for it. */
  answerWaiting?: boolean;
}

/** How much of the stage the sphere fills. The voice swell adds on top. */
export const ORB_SCALE = {
  rest: 0.52,
  held: 0.56,
  wake: 0.64,
  work: 0.7,
  voice: 0.76,
  bloom: 0.84,
} as const;

/** The pulse period while Juno works, and how much a finished step cuts it. */
const WORK_PULSE_S = 1.4;
const STEP_SPEEDUP = 0.2;
const MAX_STEPS_COUNTED = 4;

const EMBER: OrbLook = {
  motion: "ember",
  hue: HUE_BASE,
  sat: 0.35,
  mono: 0,
  bright: 0.55,
  scale: ORB_SCALE.rest,
  ripple: 0,
  flow: 0.08,
  spin: 0,
  pulsePeriod: 0,
};

/** What the sphere does for a bar state. */
export function orbLook({
  state,
  stepsDone = 0,
  approvalPending = false,
  driving = false,
  answerWaiting = false,
}: OrbLookInput): OrbLook {
  if (approvalPending) {
    // Everything stops. A sphere that has stopped moving is the clearest way
    // to say the next move is yours.
    return {
      motion: "hold",
      hue: HUE_BASE,
      sat: 1,
      mono: 0,
      bright: 1,
      scale: ORB_SCALE.work,
      ripple: 0,
      flow: 0,
      spin: 0,
      pulsePeriod: 0,
    };
  }

  const steps = Math.min(MAX_STEPS_COUNTED, Math.max(0, Math.trunc(stepsDone)));
  const work: OrbLook = {
    motion: "turn",
    hue: HUE_BASE,
    sat: 1,
    mono: 0,
    bright: 1,
    scale: ORB_SCALE.work,
    ripple: 0.12,
    flow: 1 + 0.3 * steps,
    spin: 0.45 + 0.12 * steps,
    pulsePeriod: WORK_PULSE_S / (1 + STEP_SPEEDUP * steps),
  };

  if (driving) return { ...work, pulsePeriod: 0, spin: 0.45 };

  const rest: OrbLook = answerWaiting
    ? { ...EMBER, hue: HUE_GREEN, sat: 0.5, bright: 0.7, scale: ORB_SCALE.held }
    : EMBER;

  switch (state) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_SHRINKING:
      return rest;
    case UI.BAR_STATES_DICTATION_READY:
      // Dictation is armed: the ember takes the colour your words will have.
      return { ...rest, hue: HUE_GREEN, sat: 0.5, bright: 0.6, scale: ORB_SCALE.rest };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      // Waiting for the wake word: cool, and dimmer than rest, so a desk with
      // always-listening on does not glow at you all day.
      return { ...rest, hue: HUE_COOL, sat: 0.45, bright: 0.4, scale: 0.5 };
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_INPUT:
      return {
        motion: "wake",
        hue: HUE_BASE,
        sat: 0.8,
        mono: 0,
        bright: 0.9,
        scale: ORB_SCALE.wake,
        ripple: 0.08,
        flow: 0.35,
        spin: 0,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_LISTENING:
      return {
        motion: "hear",
        hue: HUE_COOL,
        sat: 1,
        mono: 0,
        bright: 1,
        scale: ORB_SCALE.voice,
        ripple: 0.22,
        flow: 0.7,
        spin: 0.1,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_DICTATING:
      return {
        motion: "hear",
        hue: HUE_GREEN,
        sat: 1,
        mono: 0,
        bright: 1,
        scale: ORB_SCALE.voice,
        ripple: 0.22,
        flow: 0.7,
        spin: 0.1,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_TRANSCRIBING:
      // Still your words, so still green; turning, because Juno has them now.
      return { ...work, hue: HUE_GREEN };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return work;
    case UI.BAR_STATES_SPEAKING:
      return {
        motion: "voice",
        hue: HUE_BASE,
        sat: 1,
        mono: 0,
        bright: 1,
        scale: ORB_SCALE.voice,
        ripple: 0.18,
        flow: 0.9,
        spin: 0.2,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return {
        motion: "bloom",
        hue: HUE_GREEN,
        sat: 1,
        mono: 0,
        bright: 1,
        scale: ORB_SCALE.bloom,
        ripple: 0.1,
        flow: 0.6,
        spin: 0.1,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_ERROR:
      // The one place the three colours collapse into one: a single red
      // object, because a failure must not be read as a mood.
      return {
        motion: "recoil",
        hue: HUE_BASE,
        sat: 1,
        mono: 1,
        bright: 1,
        scale: ORB_SCALE.work,
        ripple: 0.06,
        flow: 0.4,
        spin: 0,
        pulsePeriod: 0,
      };
    case UI.BAR_STATES_STOPPING:
      return { ...EMBER, bright: 0.7, sat: 0.4, scale: 0.66, flow: 0.25, spin: 0.12 };
    default:
      return rest;
  }
}

/** Whether the frame loop may sleep in this look once it has settled. */
export function loopMaySleep(look: OrbLook): boolean {
  return look.motion === "ember" || look.motion === "hold";
}

/** One-shot motions. They play on entry and must not repeat while held. */
export function isImpulse(motion: OrbMotion): boolean {
  return motion === "recoil" || motion === "bloom";
}

/** The values the canvas eases toward, every frame. Pure, so the easing and
 *  the sleep rule are both testable without a GPU. */
export interface OrbTargets {
  hue: number;
  sat: number;
  mono: number;
  bright: number;
  scale: number;
  ripple: number;
  flow: number;
  spin: number;
}

/** What the audio level does to a look. Only your voice and Juno's move it. */
export function orbTargets(look: OrbLook, audioLevel: number): OrbTargets {
  const level = Number.isFinite(audioLevel) ? Math.min(1, Math.max(0, audioLevel)) : 0;
  const base: OrbTargets = {
    hue: look.hue,
    sat: look.sat,
    mono: look.mono,
    bright: look.bright,
    scale: look.scale,
    ripple: look.ripple,
    flow: look.flow,
    spin: look.spin,
  };
  switch (look.motion) {
    case "hear":
      return {
        ...base,
        scale: look.scale + 0.18 * level,
        ripple: look.ripple + 0.5 * level,
        flow: look.flow + 0.3 * level,
      };
    case "voice":
      return {
        ...base,
        scale: look.scale + 0.06 * level,
        ripple: look.ripple + 0.45 * level,
        flow: look.flow + 0.25 * level,
      };
    default:
      return base;
  }
}

/** The shortest way round from one hue to another, in degrees. */
export function hueDelta(from: number, to: number): number {
  const d = ((to - from) % 360 + 540) % 360 - 180;
  return d;
}

/** Close enough that the loop can stop drawing without a visible step. */
export function settled(current: OrbTargets, target: OrbTargets): boolean {
  return (
    Math.abs(hueDelta(current.hue, target.hue)) < 0.5 &&
    Math.abs(current.sat - target.sat) < 0.005 &&
    Math.abs(current.mono - target.mono) < 0.005 &&
    Math.abs(current.bright - target.bright) < 0.005 &&
    Math.abs(current.scale - target.scale) < 0.002 &&
    Math.abs(current.ripple - target.ripple) < 0.005 &&
    Math.abs(current.flow - target.flow) < 0.01 &&
    Math.abs(current.spin - target.spin) < 0.01
  );
}

// === THE WORDS ===

/**
 * The Orb says everything with the sphere, so words are not a running
 * commentary: they appear only when Juno needs an answer from you, when
 * something failed, or when you asked to type. Never a status word, never
 * your transcript, never the answer as subtitles. That is Presence's job.
 */
export type Words =
  | { kind: "composer" }
  | { kind: "approval"; text: string }
  | { kind: "error"; text: string };

export interface WordsInput {
  state: string;
  currentError: string | null;
  /** A tool waiting on Allow or Don't, described. */
  approval?: string | null;
}

export function wordsFor({ state, currentError, approval }: WordsInput): Words | null {
  if (approval) return { kind: "approval", text: `Juno wants to ${cleanTitle(approval)}` };
  if (state === UI.BAR_STATES_ERROR) {
    return { kind: "error", text: currentError || "Something went wrong" };
  }
  if (isInputState(state)) return { kind: "composer" };
  return null;
}

function cleanTitle(text: string): string {
  return text.trim().replace(/\.$/, "");
}

// === THE WINDOW ===

/** What sits under the sphere, which decides the window. */
export type Posture = "orb" | "words" | "approval" | "sheet";

/**
 * An approval outranks everything: it is the only thing that cannot wait.
 * Then the sheet you opened, then a line, then the sphere alone.
 */
export function posture(words: Words | null, sheetOpen: boolean): Posture {
  if (words?.kind === "approval") return "approval";
  if (sheetOpen) return "sheet";
  if (words) return "words";
  return "orb";
}

/** The window for a posture. The sphere's centre is at (width/2, STAGE/2) in
 *  every one of them, so a centre-stable, top-anchored resize never moves it. */
export function windowSize(p: Posture, sheetHeight = SHEET_MIN_HEIGHT): { width: number; height: number } {
  if (p === "orb") return { width: STAGE, height: STAGE };
  const width = CAPTION_WIDTH + 2 * SIDE_PAD;
  const below =
    p === "approval" ? APPROVAL_HEIGHT : p === "words" ? CAPTION_HEIGHT : sheetHeightFor(sheetHeight);
  return { width, height: STAGE + PANEL_GAP + below + BOTTOM_PAD };
}

// === WHAT THE SHEET HOLDS ===

/**
 * The sheet is the Orb's only way to read, so it holds the whole turn: what
 * Juno heard, dimmed, and the answer under it. Showing the question is what
 * makes a misheard query recoverable in one click, since the sphere never
 * showed a transcript.
 */
export interface SheetContent {
  question: string;
  answer: ChatMessage | null;
  streaming: boolean;
}

/** Whether there is anything worth unfolding. */
export function hasSheet(content: SheetContent, noticeOpen = false): boolean {
  if (noticeOpen) return true;
  if (content.answer && content.answer.content.trim().length > 0) return true;
  return content.question.trim().length > 0;
}

/** Whether an answer must open the sheet by itself: a component cannot be
 *  spoken, so an orb that stayed shut would have swallowed it. */
export function mustOpenSheet(answer: ChatMessage | null): boolean {
  if (!answer) return false;
  return !!answer.isJsx || /<(?!TTS\b)[A-Z][A-Za-z0-9]*(\s|>|\/)/.test(answer.content);
}
