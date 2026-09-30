import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

/**
 * The Halo's model: pure functions from what Rust says (the bar state, the
 * mic level) and what the ring knows locally (the turn, a linger clock) to
 * what the ring draws. Nothing here touches the DOM or Tauri, so every row of
 * the table in docs/plans/halo-appearance.md is a unit test.
 *
 * The ring is a gauge. It has four verbs: fill (your level), travel (Juno is
 * working), pulse (Juno is speaking) and close (the turn is done). Ticks and
 * the gap are marks on the same ring, not new motions.
 */

// === GEOMETRY ===

/** The ring's square, in logical px. The disc behind the ring fills it. */
export const RING_BOX = 96;
/** Centre to the middle of the stroke. */
export const RING_RADIUS = 44;
/** Empty space around the disc for the shadow and the speaking pulses. */
export const SHADOW_PAD = 20;
/** The sheet that slides out from under the disc. */
export const SHEET_WIDTH = 320;
export const SHEET_RADIUS = 18;
/** How far the sheet's content starts below the disc's centre. */
export const SHEET_TOP_INSET = RING_BOX / 2 + 10;
export const SHEET_BOTTOM_PAD = 12;
/** The sheet scrolls past this. */
export const SHEET_MAX_CONTENT = 300;
/** Sheet height follows content in these steps, so a streaming answer does
 *  not resize the window on every glyph. */
export const SHEET_HEIGHT_STEP = 8;

export const CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;
/** Length of the travelling arc as a share of the circle. */
export const TRAVEL_ARC = 0.22;
/** Ticks sit like the hours on a clock. */
export const TICK_COUNT = 12;
/** The error gap, as a share of the circle. */
export const GAP_SHARE = 0.08;

/** How long a finished answer lingers before the ring drains to rest. */
export const LINGER_MS = 12_000;
/** A shrinking window waits for the sheet to fold before it snaps. */
export const SHRINK_DELAY_MS = 350;
/** The full close is shown this long, whatever Rust says next. */
export const CLOSE_MS = 600;

/** macOS system blue, the only accent the ring uses. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";
const WHITE = "#FFFFFF";

// === STATE GROUPS ===

const INPUT_STATES: readonly string[] = [UI.BAR_STATES_INPUT, UI.BAR_STATES_EXPANDING];

const VOICE_STATES: readonly string[] = [
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

const IDLE_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
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

export function isVoiceState(state: string): boolean {
  return VOICE_STATES.includes(state);
}

export function isInputState(state: string): boolean {
  return INPUT_STATES.includes(state);
}

export function isIdleState(state: string): boolean {
  return IDLE_STATES.includes(state);
}

export function isWorkingState(state: string): boolean {
  return WORKING_STATES.includes(state);
}

// === THE RING ===

/** What the ring is doing. */
export type RingVerb = "rest" | "fill" | "travel" | "pulse" | "close" | "gap" | "drain";

export interface RingLook {
  verb: RingVerb;
  color: string;
  opacity: number;
  /** fill and drain: how much of the circle is drawn, 0..1. */
  level: number;
  /** rest: a slow breath (always listening). */
  breathe: boolean;
  /** travel: hold the arc with its leading edge at this angle (degrees from
   *  12 o'clock, clockwise) because a tool is running there. */
  holdAt: number | null;
  /** travel: turn at half speed (stopping). */
  slow: boolean;
}

const REST: RingLook = {
  verb: "rest",
  color: WHITE,
  opacity: 0.3,
  level: 1,
  breathe: false,
  holdAt: null,
  slow: false,
};

export interface RingInput {
  state: string;
  /** The mic level Rust reports, 0..1. */
  audioLevel: number;
  /** Tool calls finished in this turn: the ticks. */
  completedTools: number;
  /** A tool is running right now: the arc pauses at its tick. */
  runningTool: boolean;
  /** A tool waits on Allow or Don't: the arc pauses at its tick too. */
  approvalPending: boolean;
  /** Juno holds the physical cursor. */
  driving: boolean;
  /** The turn finished within CLOSE_MS: the ring is drawn closed once. */
  closing: boolean;
  /** A finished answer is on stage and its clock is running: 1 full, 0 empty. */
  lingerProgress: number | null;
}

/** Where tick number `index` (0-based) sits, in degrees from 12 o'clock. */
export function tickAngle(index: number): number {
  return ((index % TICK_COUNT) * 360) / TICK_COUNT;
}

/** The angles of every tick a turn has earned, at most one per hour mark. */
export function tickAngles(completed: number): number[] {
  const n = Math.max(0, Math.min(TICK_COUNT, Math.floor(completed)));
  return Array.from({ length: n }, (_, i) => tickAngle(i));
}

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, Number.isFinite(v) ? v : 0));
}

/** The ring for a bar state plus what the turn knows. */
export function ringFor(input: RingInput): RingLook {
  const { state } = input;
  if (input.closing) return { ...REST, verb: "close", color: SYSTEM_GREEN, opacity: 1 };
  if (state === UI.BAR_STATES_ERROR) return { ...REST, verb: "gap", color: WHITE, opacity: 0.6 };
  if (input.driving) return { ...REST, verb: "travel", color: SYSTEM_BLUE, opacity: 0.9 };
  const paused = input.runningTool || input.approvalPending;
  const holdAt = paused ? tickAngle(input.completedTools) : null;
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return { ...REST, verb: "fill", color: SYSTEM_BLUE, opacity: 1, level: clamp01(input.audioLevel) };
    case UI.BAR_STATES_DICTATING:
      return { ...REST, verb: "fill", color: SYSTEM_GREEN, opacity: 1, level: clamp01(input.audioLevel) };
    case UI.BAR_STATES_TRANSCRIBING:
      // The sentence is captured: the meter reads full while it is written down.
      return { ...REST, verb: "fill", color: SYSTEM_GREEN, opacity: 0.6, level: 1 };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { ...REST, color: SYSTEM_BLUE, opacity: 0.45, breathe: true };
    case UI.BAR_STATES_DICTATION_READY:
      return { ...REST, color: SYSTEM_GREEN, opacity: 0.55 };
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_INPUT:
      return { ...REST, opacity: 0.55 };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { ...REST, verb: "travel", opacity: 0.9, holdAt };
    case UI.BAR_STATES_STOPPING:
      return { ...REST, verb: "travel", opacity: 0.5, slow: true };
    case UI.BAR_STATES_SPEAKING:
      return { ...REST, verb: "pulse", opacity: 0.85 };
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_FINISHING:
      return { ...REST, verb: "close", color: SYSTEM_GREEN, opacity: 1 };
    case UI.BAR_STATES_SHRINKING:
      return { ...REST, opacity: 0.2 };
    case UI.BAR_STATES_DEFAULT:
      // A finished answer is on stage: the ring is its clock, draining to rest.
      if (input.lingerProgress !== null) {
        return { ...REST, verb: "drain", opacity: 0.4, level: clamp01(input.lingerProgress) };
      }
      return REST;
    default:
      // Anything new Rust learns to say rests until it has a row.
      return REST;
  }
}

/** Where the error gap sits: at the mark the next tool would have earned. */
export function gapAngle(completedTools: number): number {
  return tickAngle(completedTools);
}

// === THE CAPTION ===

export type CaptionTone = "live" | "dim" | "provisional" | "error" | "plain";

export interface Caption {
  text: string;
  tone: CaptionTone;
}

export interface CaptionInput {
  state: string;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  spokenText: string;
  currentError: string | null;
  /** What the person asked, from the conversation or the bar. */
  question: string;
  /** A tool Juno is running right now, if any. */
  runningTool?: string | null;
  /** Juno is driving the cursor; the label to show. */
  drivingLabel?: string | null;
  /** An answer is already on stage, inside or below: the caption steps aside. */
  answerShowing?: boolean;
  /** A tool is waiting on Allow or Don't: the approval row says what. */
  approvalPending?: boolean;
}

/**
 * The one line under the ring. Null means the ring stands alone. Sentence
 * case, like every other Juno surface.
 */
export function captionFor(input: CaptionInput): Caption | null {
  const { state } = input;
  if (input.drivingLabel) return { text: input.drivingLabel, tone: "plain" };
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return input.transcriptionText
        ? { text: input.transcriptionText, tone: input.transcriptionProvisional ? "provisional" : "live" }
        : { text: "Listening", tone: "dim" };
    case UI.BAR_STATES_DICTATING:
      return input.transcriptionText
        ? { text: input.transcriptionText, tone: input.transcriptionProvisional ? "provisional" : "live" }
        : { text: "Dictating", tone: "dim" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { text: "Listening for “hey Juno”", tone: "dim" };
    case UI.BAR_STATES_TRANSCRIBING:
      return { text: input.transcriptionText || "Transcribing", tone: "dim" };
    case UI.BAR_STATES_SUBMITTING:
      return { text: input.question || "Thinking", tone: "dim" };
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      if (input.runningTool) return { text: input.runningTool, tone: "plain" };
      if (input.answerShowing || input.approvalPending) return null;
      return { text: input.question || "Working", tone: "dim" };
    case UI.BAR_STATES_SPEAKING:
      if (input.answerShowing) return null;
      return input.spokenText ? { text: input.spokenText, tone: "dim" } : { text: "Speaking", tone: "dim" };
    case UI.BAR_STATES_ERROR:
      return { text: input.currentError || "Something went wrong", tone: "error" };
    case UI.BAR_STATES_STOPPING:
      return { text: "Stopping", tone: "dim" };
    default:
      // Finishing and success: the ring closing is the word. Idle and input
      // postures carry nothing or the composer.
      return null;
  }
}

// === THE ANSWER ===

/** An answer a ring can hold whole: at most this many characters. */
export const INSIDE_MAX_CHARS = 18;

export type Placement = "inside" | "below" | "none";

export interface PlacementInput {
  /** The visible text of the answer, or the spoken text when there is none. */
  text: string;
  streaming: boolean;
}

/** Component tags and markdown blocks never fit inside a ring. */
function needsPanel(text: string): boolean {
  return /<[A-Z]/.test(text) || /\n/.test(text.trim()) || /^\s*([-*#>]|\d+\.)\s/m.test(text) || /```/.test(text);
}

/** Strip the light markdown a one-liner tends to carry so it reads as a word. */
export function plainInside(text: string): string {
  return text
    .replace(/[*_`]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Where the answer goes. A ring shows a number, a word, a time, a yes. It
 * does not decide "inside" until the answer is complete: text that moved
 * from inside to below as it grew would jump, so a streaming answer is
 * below, or not yet shown.
 */
export function placementFor({ text, streaming }: PlacementInput): Placement {
  const trimmed = text.trim();
  if (!trimmed) return "none";
  if (needsPanel(trimmed)) return "below";
  const plain = plainInside(trimmed);
  if (plain.length > INSIDE_MAX_CHARS) return "below";
  return streaming ? "none" : "inside";
}

/** Type size for text inside the ring: a number reads big, a phrase small. */
export function insideFontSize(text: string): number {
  const n = plainInside(text).length;
  if (n <= 4) return 26;
  if (n <= 8) return 18;
  return 13;
}

// === THE WINDOW ===

export interface WindowSize {
  width: number;
  height: number;
}

/** The window with nothing under the ring: the disc plus its margin. */
export const NARROW_WINDOW: WindowSize = {
  width: RING_BOX + 2 * SHADOW_PAD,
  height: RING_BOX + 2 * SHADOW_PAD,
};

/** Clamp measured sheet content to its range, in steps. */
export function sheetContentHeight(measured: number): number {
  const stepped = Math.ceil(Math.max(0, measured) / SHEET_HEIGHT_STEP) * SHEET_HEIGHT_STEP;
  return Math.min(SHEET_MAX_CONTENT, stepped);
}

/** The sheet's own height for that much content, measured from the disc's centre. */
export function sheetHeight(contentHeight: number): number {
  return SHEET_TOP_INSET + sheetContentHeight(contentHeight) + SHEET_BOTTOM_PAD;
}

/**
 * The window for a ring with, or without, a sheet under it. The ring's
 * centre is always at (width / 2, SHADOW_PAD + RING_BOX / 2): the window
 * grows around the ring horizontally and downward from it, so the ring
 * itself never moves when a sheet opens or folds.
 */
export function windowFor(sheetOpen: boolean, contentHeight = 0): WindowSize {
  if (!sheetOpen) return { ...NARROW_WINDOW };
  return {
    width: SHEET_WIDTH + 2 * SHADOW_PAD,
    height: SHADOW_PAD + RING_BOX / 2 + sheetHeight(contentHeight) + SHADOW_PAD,
  };
}

export interface LingerInput {
  /** An answer is on stage, inside or below. */
  answerShowing: boolean;
  /** The answer is still arriving. */
  streaming: boolean;
  /** Rust is still working (a later step may follow the first answer). */
  working: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
}

/** Whether the linger clock should be counting down at all. */
export function lingerShouldRun({ answerShowing, streaming, working, approvalPending }: LingerInput): boolean {
  return answerShowing && !streaming && !working && !approvalPending;
}

// === THE CURRENT TURN ===

export interface Turn {
  /** What the person asked. Empty when the conversation has no user message. */
  question: string;
  /** The reply to it, once any of it exists (visible content or spoken text). */
  answer: ChatMessage | null;
  /** A tool waiting on Allow or Don't. */
  approval: ChatMessage | null;
  /** A tool running right now, described for the caption. */
  runningTool: string | null;
  /** Tool calls that finished in this turn: one tick each. */
  completedTools: number;
}

/**
 * The ring shows the current turn, not the history: the last question and
 * everything that has happened since it.
 */
export function latestTurn(messages: ChatMessage[]): Turn {
  let start = -1;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role === "user") {
      start = i;
      break;
    }
  }
  const question = start >= 0 ? messages[start].content : "";
  let answer: ChatMessage | null = null;
  let approval: ChatMessage | null = null;
  let runningTool: string | null = null;
  let completedTools = 0;
  for (let i = start + 1; i < messages.length; i += 1) {
    const m = messages[i];
    if (m.role === "assistant" && !m.notice) {
      const hasContent = m.content.trim().length > 0 || !!m.tts_metadata?.has_spoken_content;
      if (hasContent) answer = m;
    } else if (m.role === "tool_call_request") {
      if (m.approval_state === "pending") {
        approval = m;
      } else if (m.approval_state === "denied") {
        runningTool = null;
      } else if (m.success === undefined) {
        runningTool = m.content || m.tool_name || null;
      } else {
        runningTool = null;
        completedTools += 1;
      }
    } else if (m.role === "tool_call_result") {
      completedTools += 1;
    }
  }
  return { question, answer, approval, runningTool, completedTools };
}

/** A stable identity for an answer, so a new one can be told from growth of the old. */
export function answerKey(turn: Turn): string {
  const a = turn.answer;
  if (!a) return "";
  return a.messageId ?? String(a.timestamp ?? "");
}

/** The words an answer shows: visible text first, the spoken sentence when there is none. */
export function answerText(answer: ChatMessage | null): { visible: string; spoken: string } {
  const visible = answer?.content.trim() ?? "";
  const spoken =
    answer?.tts_metadata?.total_spoken_text?.trim() ||
    answer?.tts_metadata?.tts_parts?.map((p) => p.trim()).join(" ") ||
    "";
  return { visible, spoken };
}
