import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import type { SteadySpec } from "@/lib/steadyFrame";

/**
 * The Island's model: pure functions from what Rust says (the bar state) and
 * what the island knows locally (is an answer open, is there a draft) to what
 * is on screen. Nothing here touches the DOM or Tauri, so every row of the
 * state table in docs/plans/island-appearance.md is a unit test.
 *
 * The Island is one surface with four faces. The shape morphs between them,
 * the content crossfades, and the composer is one element that is never
 * unmounted.
 */

/** The four faces. */
export type Face = "idle" | "compose" | "answer" | "component";

/** Empty space around the shape so its shadow is not clipped by the window. */
export const SHADOW_PAD = 24;

/** How long a finished answer lingers before the island settles back. */
export const LINGER_MS = 12_000;

export interface ShapeSize {
  width: number;
  height: number;
  radius: number;
}

/** Every face past idle is this wide, so the composer never changes width. */
export const ISLAND_WIDTH = 360;

/** The resting capsule: a mark, not a bar. */
export const IDLE_SHAPE: ShapeSize = { width: 92, height: 28, radius: 14 };

/** The capsule with its controls showing: barely bigger, so it reads as the same object. */
export const HOVER_SHAPE: ShapeSize = { width: 132, height: 32, radius: 16 };

/** One line of the composer, in px. */
export const COMPOSER_LINE_PX = 18;
/** The composer grows with its text up to four lines, then scrolls. */
export const COMPOSER_MAX_TEXT_PX = 4 * COMPOSER_LINE_PX;
/** Room above and below the composer's text. */
export const COMPOSER_PAD_Y = 9;

/** The composer's height for a measured text height. */
export function composerHeightFor(textHeight: number): number {
  const text = Math.min(COMPOSER_MAX_TEXT_PX, Math.max(COMPOSER_LINE_PX, textHeight));
  return text + 2 * COMPOSER_PAD_Y;
}

/** The tallest the shape gets. Taller answers scroll inside it. */
export const SHAPE_MAX_HEIGHT = 400;

/** Room beyond the composer for the skill suggestions. */
export const SUGGESTION_ROOM = 200;

/**
 * The Island's steady frame. The window holds the largest face (the full
 * answer, its shadow, and the skill suggestions) and never changes size.
 * Wells are computed for the resting capsule plus its shadow, so the capsule
 * sits exactly on the well. The preview frames the compose face.
 */
export const ISLAND_STEADY_SPEC: SteadySpec = {
  rest: { width: IDLE_SHAPE.width + 2 * SHADOW_PAD, height: IDLE_SHAPE.height + 2 * SHADOW_PAD },
  max: {
    width: ISLAND_WIDTH + 2 * SHADOW_PAD,
    height: SHAPE_MAX_HEIGHT + 2 * SHADOW_PAD + SUGGESTION_ROOM,
  },
  stage: {
    width: ISLAND_WIDTH + 2 * SHADOW_PAD,
    height: COMPOSER_LINE_PX + 2 * COMPOSER_PAD_Y + 2 * SHADOW_PAD,
  },
};

// === BAR STATES ===

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

/**
 * States in which Rust is at rest. Always listening is the wake phrase armed:
 * a standing intent Rust re-arms after a stop, not an activity.
 */
const RESTING_STATES: readonly string[] = [...IDLE_STATES, UI.BAR_STATES_ALWAYS_LISTENING];

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

export function isRestingState(state: string): boolean {
  return RESTING_STATES.includes(state);
}

export function isWorkingState(state: string): boolean {
  return WORKING_STATES.includes(state);
}

/**
 * A microphone is open on a turn. TRANSCRIBING counts only while live words
 * still arrive (provisional); once the mic has closed it is the final decode.
 * Same rule as `isRecordingTurn` in `lib/voiceTurn.ts`.
 */
export function isRecording(state: string, provisional?: boolean): boolean {
  if (state === UI.BAR_STATES_LISTENING || state === UI.BAR_STATES_DICTATING) return true;
  return state === UI.BAR_STATES_TRANSCRIBING && provisional === true;
}

// === FACES ===

export interface FaceInput {
  state: string;
  /** An answer (or a tool waiting on Allow) is open and has something to show. */
  answerOpen: boolean;
  /** The open answer is a component, and it has not been stepped back from. */
  componentShown: boolean;
  /** The composer holds a draft, has the caret, or was asked to stay open. */
  composerActive: boolean;
  /**
   * The voice session is a dictation: words typed into whatever has focus.
   * It leaves an open answer where it is, because the caret it is typing into
   * may be the follow-up field under that answer.
   */
  dictation: boolean;
  /**
   * The composer was just put away (Escape, its close control) and Rust has
   * not left its input state yet. Without this the box would flash back for
   * the moment Rust takes to shrink.
   */
  dismissed?: boolean;
}

/**
 * Which face a combination of backend state and local state gets.
 *
 * Past idle the island is the text box (the Pill's rule): every state Rust
 * can be in that is not rest is the compose face, its placeholder saying what
 * is happening. An open answer wins over that, except when a spoken turn to
 * Juno starts, which turns the answer back into the composer with the draft
 * kept. At rest the island is the capsule unless the composer is in use.
 */
export function faceFor({
  state,
  answerOpen,
  componentShown,
  composerActive,
  dictation,
  dismissed = false,
}: FaceInput): Face {
  if (dismissed && isInputState(state) && !answerOpen) return "idle";
  const spokenTurn = isVoiceState(state) && !isRestingState(state) && !dictation;
  if (answerOpen && !spokenTurn) return componentShown ? "component" : "answer";
  if (isRestingState(state)) return composerActive ? "compose" : "idle";
  return "compose";
}

export interface ShapeInput {
  hovered: boolean;
  /** The composer's own height (`composerHeightFor`). */
  composerHeight: number;
  /** The measured height of what sits beside the composer in the answer and component faces. */
  contentHeight: number;
}

/** The shape's size for a face. */
export function shapeFor(face: Face, { hovered, composerHeight, contentHeight }: ShapeInput): ShapeSize {
  if (face === "idle") return { ...(hovered ? HOVER_SHAPE : IDLE_SHAPE) };
  if (face === "compose") return { width: ISLAND_WIDTH, height: composerHeight, radius: 18 };
  const height = Math.min(SHAPE_MAX_HEIGHT, Math.ceil(composerHeight + Math.max(0, contentHeight)));
  return { width: ISLAND_WIDTH, height: Math.max(composerHeight, height), radius: 20 };
}

/** One line of words and how they read. */
export type WordsTone = "live" | "dim" | "provisional" | "error" | "plain";

export interface Words {
  text: string;
  tone: WordsTone;
}

export interface PlaceholderInput {
  state: string;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  spokenText: string;
  currentError: string | null;
  /** A turn is in flight as the island knows it. */
  working: boolean;
  /** An answer is on screen beside the composer. */
  answerShown: boolean;
  /** A tool Juno is running right now, if any. */
  runningTool?: string | null;
  /** Juno is driving the cursor; what it is doing. */
  drivingLabel?: string | null;
}

/**
 * What the empty composer says. The box stays up through every state, so it
 * carries what a status line would have said: the mic is open (with your
 * words as they arrive, for display only), the words are being decoded, Juno
 * is working, something went wrong. Sentence case, like every Juno surface.
 */
export function composerPlaceholder(input: PlaceholderInput): Words {
  const { state } = input;
  if (input.drivingLabel) return { text: `Juno is ${input.drivingLabel}`, tone: "plain" };
  if (isRecording(state, input.transcriptionProvisional)) {
    return input.transcriptionText
      ? { text: input.transcriptionText, tone: input.transcriptionProvisional ? "provisional" : "live" }
      : { text: "Listening…", tone: "dim" };
  }
  switch (state) {
    case UI.BAR_STATES_TRANSCRIBING:
      return { text: input.transcriptionText || "Transcribing…", tone: "dim" };
    case UI.BAR_STATES_ERROR:
      return { text: input.currentError || "Something went wrong", tone: "error" };
    case UI.BAR_STATES_SUBMITTING:
      return { text: "Sending…", tone: "dim" };
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { text: input.runningTool ? `${input.runningTool}…` : "Working…", tone: "dim" };
    case UI.BAR_STATES_SPEAKING:
      return input.answerShown
        ? { text: "Follow up", tone: "dim" }
        : { text: input.spokenText || "Speaking…", tone: "dim" };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return { text: input.answerShown ? "Follow up" : "Done", tone: "dim" };
    case UI.BAR_STATES_STOPPING:
      return { text: "Stopping…", tone: "dim" };
    default:
      if (input.working) return { text: "Working…", tone: "dim" };
      return { text: input.answerShown ? "Follow up" : "Ask Juno", tone: "dim" };
  }
}

/** What the one stop control stops right now. */
export type StopKind = "cancel" | "stop" | "close";

export const STOP_LABELS: Record<StopKind, string> = {
  cancel: "Cancel without sending",
  stop: "Stop Juno",
  close: "Close without sending",
};

/**
 * The one stop control, named for what it stops: a recording turn is
 * cancelled, running work is stopped, and with nothing running the compose
 * face is put away. Null on an answer with nothing running, which already
 * has its own close control.
 */
export function stopControl({
  face,
  state,
  provisional,
  working,
  driving,
}: {
  face: Face;
  state: string;
  provisional?: boolean;
  working: boolean;
  driving: boolean;
}): StopKind | null {
  if (isRecording(state, provisional)) return "cancel";
  if (working || driving) return "stop";
  return face === "compose" ? "close" : null;
}

export interface LingerInput {
  face: Face;
  /** The answer is still arriving. */
  streaming: boolean;
  /** Rust is still working (a later step may follow the first answer). */
  working: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
  /** The composer has text in it or the caret. */
  composerEngaged: boolean;
}

/**
 * Whether the linger ring counts down: on a finished answer or component,
 * never while the person is writing a follow-up or a tool waits on them.
 */
export function lingerShouldRun({ face, streaming, working, approvalPending, composerEngaged }: LingerInput): boolean {
  if (face !== "answer" && face !== "component") return false;
  return !streaming && !working && !approvalPending && !composerEngaged;
}

// === THE DOT ===

/** How the dot moves. One dot; no icons. */
export type DotMotion = "breathe" | "slow" | "still" | "orbit" | "ripple" | "shake" | "flash";

export interface DotLook {
  color: string;
  opacity: number;
  motion: DotMotion;
}

/** macOS system blue, the only accent the island uses. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";
const WHITE = "#FFFFFF";

/** The dot for a bar state. */
export function dotFor(state: string, driving = false): DotLook {
  if (driving) return { color: SYSTEM_BLUE, opacity: 0.9, motion: "orbit" };
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
      return { color: WHITE, opacity: 0.45, motion: "slow" };
    case UI.BAR_STATES_DICTATION_READY:
      return { color: SYSTEM_GREEN, opacity: 0.6, motion: "still" };
    case UI.BAR_STATES_SHRINKING:
      return { color: WHITE, opacity: 0.25, motion: "still" };
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_INPUT:
      return { color: WHITE, opacity: 0.7, motion: "still" };
    case UI.BAR_STATES_LISTENING:
      return { color: SYSTEM_BLUE, opacity: 1, motion: "breathe" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { color: SYSTEM_BLUE, opacity: 0.4, motion: "slow" };
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_TRANSCRIBING:
      return { color: SYSTEM_GREEN, opacity: 1, motion: "breathe" };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { color: WHITE, opacity: 0.85, motion: "orbit" };
    case UI.BAR_STATES_STOPPING:
      return { color: WHITE, opacity: 0.5, motion: "orbit" };
    case UI.BAR_STATES_SPEAKING:
      return { color: WHITE, opacity: 0.85, motion: "ripple" };
    case UI.BAR_STATES_ERROR:
      return { color: SYSTEM_RED, opacity: 1, motion: "shake" };
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_FINISHING:
      return { color: SYSTEM_GREEN, opacity: 1, motion: "flash" };
    default:
      return { color: WHITE, opacity: 0.35, motion: "still" };
  }
}

// === THE NOTCH'S ONE LINE ===

export interface WordsInput {
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
}

/**
 * The one line inside the Notch's ear and status postures (`../notch/`).
 * Null means the posture shows the dot alone. The Island itself says these
 * things in its composer's placeholder (`composerPlaceholder`).
 */
export function wordsFor(input: WordsInput): Words | null {
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
      return { text: input.question || "Working", tone: "dim" };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return { text: "Done", tone: "plain" };
    case UI.BAR_STATES_SPEAKING:
      return input.spokenText ? { text: input.spokenText, tone: "dim" } : { text: "Speaking", tone: "dim" };
    case UI.BAR_STATES_ERROR:
      return { text: input.currentError || "Something went wrong", tone: "error" };
    case UI.BAR_STATES_STOPPING:
      return { text: "Stopping", tone: "dim" };
    default:
      return null;
  }
}

// === THE CURRENT TURN ===

export interface Turn {
  /** What the person asked. Empty when the conversation has no user message. */
  question: string;
  /** The reply to it, once any of it exists (visible content or spoken text). */
  answer: ChatMessage | null;
  /** A tool waiting on Allow or Don't. */
  approval: ChatMessage | null;
  /** A tool running right now, described for the placeholder. */
  runningTool: string | null;
}

/**
 * The island shows the current turn, not the history: the last question and
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
  for (let i = start + 1; i < messages.length; i += 1) {
    const m = messages[i];
    if (m.role === "assistant" && !m.notice) {
      const hasContent = m.content.trim().length > 0 || !!m.tts_metadata?.has_spoken_content;
      if (hasContent) answer = m;
    } else if (m.role === "tool_call_request") {
      if (m.approval_state === "pending") {
        approval = m;
      } else if (m.success === undefined && m.approval_state !== "denied") {
        runningTool = m.content || m.tool_name || null;
      } else {
        runningTool = null;
      }
    }
  }
  return { question, answer, approval, runningTool };
}

/** A stable identity for an answer, so a new one can be told from growth of the old. */
export function answerKey(turn: Turn): string {
  const a = turn.answer;
  if (!a) return "";
  return a.messageId ?? String(a.timestamp ?? "") ?? "";
}
