import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

/**
 * The Orb's model: pure functions from what Rust says (the bar state, the
 * audio level) and what the orb knows locally (the current turn, a hover, an
 * approval) to what the orb does and what the one caption under it says.
 * Nothing here touches the DOM, Three.js or Tauri, so every row of the state
 * table in docs/plans/orb-appearance.md is a unit test.
 */

// === GEOMETRY ===

/** The canvas is this big, once. The orb never resizes it; it scales itself. */
export const STAGE = 120;

/** Room beside the caption and the sheet for their shadow. */
export const SIDE_PAD = 20;
/** Room under the caption or the sheet. */
export const BOTTOM_PAD = 16;
/** Gap between the orb's stage and the caption. */
export const CAPTION_GAP = 4;

/** The caption pill and the sheet share one width so the window never lurches. */
export const CAPTION_WIDTH = 320;
export const CAPTION_HEIGHT = 30;
/** An approval is the caption plus a row of two buttons. */
export const APPROVAL_HEIGHT = 66;
export const SHEET_MIN_HEIGHT = 56;
export const SHEET_MAX_HEIGHT = 320;
/** Sheet height follows its content in steps, so a streaming answer does not
 *  resize the window on every glyph. */
export const SHEET_HEIGHT_STEP = 8;

/** How long a finished answer's caption and sheet linger before they fade. */
export const LINGER_MS = 10_000;
/** A shrinking window waits for the caption's fade before it snaps. */
export const SHRINK_DELAY_MS = 350;
/** The sheet opens this long after the pointer arrives, and closes this long
 *  after it leaves, so passing over the orb does not flap the window. */
export const HOVER_OPEN_MS = 150;
export const HOVER_CLOSE_MS = 400;

// === COLOUR ===

/** macOS system blue: the only accent. Juno's turn, or Juno listening. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";

/** Two tones the shader ramps between; the second is the lighter one. */
export type Tint = readonly [string, string];

export const TINT_REST: Tint = ["#6E6E73", "#AEAEB2"];
export const TINT_BLUE: Tint = [SYSTEM_BLUE, "#8EC5FF"];
export const TINT_BLUE_DEEP: Tint = ["#0066D6", "#5FA8FF"];
export const TINT_GREEN: Tint = [SYSTEM_GREEN, "#A6EFB8"];
export const TINT_RED: Tint = [SYSTEM_RED, "#FFB3AE"];

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

// === THE ORB ===

/** How the orb moves, beyond what the audio does to it. */
export type OrbMotion =
  /** Slow breath; the loop sleeps once the colours settle. */
  | "breathe"
  /** Holds still: an approval is waiting on you. */
  | "still"
  /** Swells with your voice. */
  | "swell"
  /** Steady inner rotation with a short pulse; faster as steps complete. */
  | "spin"
  /** Ripples with Juno's speech. */
  | "speak"
  /** One red flinch, then holds. */
  | "flinch"
  /** One green bloom, then settles. */
  | "bloom";

export interface OrbLook {
  tint: Tint;
  /** Size relative to the stage: 1 fills it. */
  scale: number;
  /** 1 is fully there; the orb dims to rest. */
  opacity: number;
  motion: OrbMotion;
  /** How fast the inner flow turns, in flow units per second. */
  spin: number;
  /** Pulse period in seconds; 0 means no pulse. */
  pulsePeriod: number;
}

export interface OrbLookInput {
  state: string;
  /** Tool calls finished in this turn; the spin speeds up with each. */
  stepsDone?: number;
  /** A tool is waiting on Allow or Don't. */
  approvalPending?: boolean;
  /** Juno holds the physical cursor. */
  driving?: boolean;
}

/** Sizes as a fraction of the stage. The swell adds up to 0.2 on top. */
export const ORB_SCALE = {
  rest: 0.5,
  awake: 0.62,
  voice: 0.78,
  work: 0.7,
} as const;

/** The pulse period while Juno works, and how much each finished step shortens it. */
const WORK_PULSE_S = 1.2;
const STEP_SPEEDUP = 0.25;
const MAX_STEPS_COUNTED = 4;

/** What the orb does for a bar state. */
export function orbLook({
  state,
  stepsDone = 0,
  approvalPending = false,
  driving = false,
}: OrbLookInput): OrbLook {
  if (approvalPending) {
    return { tint: TINT_BLUE, scale: ORB_SCALE.work, opacity: 1, motion: "still", spin: 0, pulsePeriod: 0 };
  }
  if (driving) {
    return { tint: TINT_BLUE_DEEP, scale: ORB_SCALE.work, opacity: 1, motion: "spin", spin: 1, pulsePeriod: WORK_PULSE_S };
  }
  const steps = Math.min(MAX_STEPS_COUNTED, Math.max(0, stepsDone));
  const work: OrbLook = {
    tint: TINT_BLUE_DEEP,
    scale: ORB_SCALE.work,
    opacity: 1,
    motion: "spin",
    spin: 0.8 + STEP_SPEEDUP * steps,
    pulsePeriod: WORK_PULSE_S / (1 + STEP_SPEEDUP * steps),
  };
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_SHRINKING:
      return { tint: TINT_REST, scale: ORB_SCALE.rest, opacity: 0.7, motion: "breathe", spin: 0, pulsePeriod: 0 };
    case UI.BAR_STATES_DICTATION_READY:
      return { tint: TINT_GREEN, scale: ORB_SCALE.rest, opacity: 0.7, motion: "breathe", spin: 0, pulsePeriod: 0 };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { tint: TINT_BLUE, scale: ORB_SCALE.rest, opacity: 0.55, motion: "breathe", spin: 0, pulsePeriod: 0 };
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_INPUT:
      return { tint: TINT_REST, scale: ORB_SCALE.awake, opacity: 1, motion: "breathe", spin: 0, pulsePeriod: 0 };
    case UI.BAR_STATES_LISTENING:
      return { tint: TINT_BLUE, scale: ORB_SCALE.voice, opacity: 1, motion: "swell", spin: 0.35, pulsePeriod: 0 };
    case UI.BAR_STATES_DICTATING:
      return { tint: TINT_GREEN, scale: ORB_SCALE.voice, opacity: 1, motion: "swell", spin: 0.35, pulsePeriod: 0 };
    case UI.BAR_STATES_TRANSCRIBING:
      return { ...work, tint: TINT_GREEN };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return work;
    case UI.BAR_STATES_SPEAKING:
      return { tint: TINT_BLUE, scale: ORB_SCALE.voice, opacity: 1, motion: "speak", spin: 0.6, pulsePeriod: 0 };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return { tint: TINT_GREEN, scale: ORB_SCALE.work, opacity: 1, motion: "bloom", spin: 0.3, pulsePeriod: 0 };
    case UI.BAR_STATES_ERROR:
      return { tint: TINT_RED, scale: ORB_SCALE.work, opacity: 1, motion: "flinch", spin: 0, pulsePeriod: 0 };
    case UI.BAR_STATES_STOPPING:
      return { tint: TINT_REST, scale: ORB_SCALE.work, opacity: 0.8, motion: "spin", spin: 0.3, pulsePeriod: 0 };
    default:
      return { tint: TINT_REST, scale: ORB_SCALE.rest, opacity: 0.7, motion: "breathe", spin: 0, pulsePeriod: 0 };
  }
}

/** Whether the frame loop may sleep in this look once it has settled. */
export function loopMaySleep(look: OrbLook): boolean {
  return look.motion === "breathe" || look.motion === "still";
}

/** The per-frame targets the canvas eases toward. Pure, so the easing can be tested. */
export interface OrbTargets {
  scale: number;
  /** Ring bulge and lobe height: 0 calm, 1 full voice. */
  swell: number;
  /** Flow distortion: 0 calm, 1 churning. */
  turbulence: number;
}

/** What the audio level does to a look. Only voice and speech follow it. */
export function orbTargets(look: OrbLook, audioLevel: number): OrbTargets {
  const level = Number.isFinite(audioLevel) ? Math.min(1, Math.max(0, audioLevel)) : 0;
  switch (look.motion) {
    case "swell":
      return { scale: look.scale + 0.2 * level, swell: level, turbulence: 0.2 + 0.3 * level };
    case "speak":
      return { scale: look.scale + 0.08 * level, swell: 0.15 * level, turbulence: 0.35 + 0.65 * level };
    case "spin":
      return { scale: look.scale, swell: 0, turbulence: 0.45 };
    case "still":
      return { scale: look.scale, swell: 0, turbulence: 0 };
    default:
      return { scale: look.scale, swell: 0, turbulence: 0.12 };
  }
}

/** Close enough that the loop can stop drawing without a visible step. */
export function settled(current: OrbTargets & { opacity: number }, target: OrbTargets & { opacity: number }): boolean {
  return (
    Math.abs(current.scale - target.scale) < 0.002 &&
    Math.abs(current.swell - target.swell) < 0.005 &&
    Math.abs(current.turbulence - target.turbulence) < 0.005 &&
    Math.abs(current.opacity - target.opacity) < 0.005
  );
}

// === THE CAPTION ===

export type CaptionTone = "live" | "dim" | "provisional" | "error" | "plain";

export type Caption =
  | { kind: "words"; text: string; tone: CaptionTone }
  | { kind: "composer" }
  | { kind: "approval"; text: string };

export interface CaptionInput {
  state: string;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  /** The sentence Rust is speaking right now. */
  spokenText: string;
  currentError: string | null;
  /** What the person asked, from the conversation or the bar. */
  question: string;
  /** The answer so far, if any: its spoken parts and its visible text. */
  answer?: { spokenParts: string[]; visibleText: string; streaming: boolean } | null;
  /** A tool running right now, described. */
  runningTool?: string | null;
  /** A tool waiting on Allow or Don't, described. */
  approval?: string | null;
  /** Juno is driving the cursor; the label to show. */
  drivingLabel?: string | null;
  /** A finished answer is still lingering under the orb. */
  lingering?: boolean;
}

/**
 * The one line under the orb. Null means the orb stands alone. Sentence
 * case; says what is happening, never what the software is doing.
 */
export function captionFor(input: CaptionInput): Caption | null {
  const { state } = input;
  if (input.approval) return { kind: "approval", text: `Juno wants to ${cleanTitle(input.approval)}` };
  if (input.drivingLabel) return { kind: "words", text: `Juno is ${input.drivingLabel}`, tone: "plain" };
  if (state === UI.BAR_STATES_ERROR) {
    return { kind: "words", text: input.currentError || "Something went wrong", tone: "error" };
  }
  if (isInputState(state)) return { kind: "composer" };

  const yourWords = (): Caption | null =>
    input.transcriptionText
      ? {
          kind: "words",
          text: input.transcriptionText,
          tone: input.transcriptionProvisional ? "provisional" : "live",
        }
      : null;
  const said = answerCaption(input.answer);
  const working = (): Caption | null => {
    if (input.runningTool) return { kind: "words", text: cleanTitle(input.runningTool), tone: "plain" };
    if (said) return said;
    return input.question ? { kind: "words", text: input.question, tone: "dim" } : null;
  };

  switch (state) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
      return yourWords();
    case UI.BAR_STATES_TRANSCRIBING:
      return input.transcriptionText
        ? { kind: "words", text: input.transcriptionText, tone: "dim" }
        : null;
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return working();
    case UI.BAR_STATES_SPEAKING:
      if (input.spokenText) return { kind: "words", text: input.spokenText, tone: "live" };
      return said;
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return said;
    case UI.BAR_STATES_STOPPING:
      return null;
    default:
      // At rest a finished answer keeps its last line for a while.
      return input.lingering ? said : null;
  }
}

/** The answer's current line: the latest spoken sentence, else the latest
 *  visible sentence. Null when there is nothing to say yet. */
export function answerCaption(
  answer: CaptionInput["answer"] | undefined,
): Extract<Caption, { kind: "words" }> | null {
  if (!answer) return null;
  const spoken = answer.spokenParts.filter((p) => p.trim()).at(-1)?.trim();
  if (spoken) return { kind: "words", text: spoken, tone: "live" };
  const sentence = latestSentence(answer.visibleText, answer.streaming);
  return sentence ? { kind: "words", text: sentence, tone: "live" } : null;
}

/** Strips JSX tags and markdown marks so a caption reads as one plain line. */
export function plainText(text: string): string {
  return text
    .replace(/<[A-Za-z][^>]*\/>/g, " ")
    .replace(/<[A-Za-z][^>]*>[\s\S]*?<\/[A-Za-z][^>]*>/g, " ")
    .replace(/<\/?[A-Za-z][^>]*>/g, " ")
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[*_`#>]+/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * The latest sentence of a text. While streaming, only a sentence that has
 * ended counts, so the caption never shows half a thought; once done, the
 * last fragment counts too.
 */
export function latestSentence(text: string, streaming = false): string {
  const plain = plainText(text);
  if (!plain) return "";
  const parts = plain.match(/[^.!?]+[.!?]+["')\]]*|[^.!?]+$/g) ?? [];
  const finished = parts.filter((p) => /[.!?]["')\]]*\s*$/.test(p));
  const pick = streaming ? finished.at(-1) : parts.at(-1);
  return (pick ?? "").trim();
}

function cleanTitle(text: string): string {
  return text.trim().replace(/\.$/, "");
}

// === THE WINDOW ===

/** What is under the orb, decides the window. */
export type Posture = "orb" | "caption" | "approval" | "sheet";

export function posture(caption: Caption | null, sheetOpen: boolean): Posture {
  if (sheetOpen) return "sheet";
  if (!caption) return "orb";
  if (caption.kind === "approval") return "approval";
  return "caption";
}

/** Clamp a measured sheet content height to the sheet's range, in steps. */
export function sheetHeightFor(contentHeight: number): number {
  const stepped = Math.ceil(contentHeight / SHEET_HEIGHT_STEP) * SHEET_HEIGHT_STEP;
  return Math.min(SHEET_MAX_HEIGHT, Math.max(SHEET_MIN_HEIGHT, stepped));
}

/** The window for a posture. The orb's centre is at (width/2, STAGE/2) in
 *  every one of them, so a centre-stable, top-anchored resize never moves it. */
export function windowSize(p: Posture, sheetHeight = SHEET_MIN_HEIGHT): { width: number; height: number } {
  if (p === "orb") return { width: STAGE, height: STAGE };
  const width = CAPTION_WIDTH + 2 * SIDE_PAD;
  const below =
    p === "caption" ? CAPTION_HEIGHT : p === "approval" ? APPROVAL_HEIGHT : sheetHeightFor(sheetHeight);
  return { width, height: STAGE + CAPTION_GAP + below + BOTTOM_PAD };
}

// === THE CURRENT TURN ===

export interface Turn {
  question: string;
  answer: ChatMessage | null;
  approval: ChatMessage | null;
  runningTool: string | null;
  /** Tool calls that have finished, so the orb can quicken. */
  stepsDone: number;
}

/** The orb shows the current turn: the last question and what followed it. */
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
  let stepsDone = 0;
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
        if (m.success !== undefined) stepsDone += 1;
      }
    }
  }
  return { question, answer, approval, runningTool, stepsDone };
}

/** A stable identity for an answer, so a new one can be told from growth of the old. */
export function answerKey(turn: Turn): string {
  const a = turn.answer;
  if (!a) return "";
  return a.messageId ?? String(a.timestamp ?? "");
}

/** Whether an answer has anything for the sheet beyond its caption. */
export function hasSheetContent(answer: ChatMessage | null): boolean {
  if (!answer) return false;
  return answer.content.trim().length > 0;
}

/** Whether the answer carries a component, which the sheet opens for by itself. */
export function hasComponent(answer: ChatMessage | null): boolean {
  if (!answer) return false;
  return !!answer.isJsx || /<(?!TTS\b)[A-Z][A-Za-z0-9]*(\s|>|\/)/.test(answer.content);
}
