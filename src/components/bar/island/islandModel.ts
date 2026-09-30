import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

/**
 * The Island's model: pure functions from what Rust says (the bar state) and
 * what the island knows locally (is a card open, is Juno driving) to what is
 * on screen. Nothing here touches the DOM or Tauri, so every row of the state
 * table in docs/plans/island-appearance.md is a unit test.
 */

/** The five shapes the island takes. */
export type Posture = "capsule" | "ear" | "line" | "status" | "card";

/** Empty space around the island so its shadow is not clipped by the window. */
export const SHADOW_PAD = 24;

/** How long a finished answer lingers before the island settles back. */
export const LINGER_MS = 12_000;

/** A shrinking window waits for the island's spring before it snaps. */
export const SHRINK_DELAY_MS = 350;

/** Card height follows content, in these steps, so a streaming answer does
 *  not resize the window on every glyph. */
export const CARD_HEIGHT_STEP = 8;

export interface IslandSize {
  width: number;
  height: number;
  radius: number;
}

/** Island sizes per posture. Ear and status match on purpose: the island must
 *  not lurch wider the instant someone stops talking. */
export const ISLAND_SIZES: {
  capsule: IslandSize;
  ear: IslandSize;
  line: IslandSize;
  status: IslandSize;
  card: { width: number; minHeight: number; maxHeight: number; radius: number };
} = {
  capsule: { width: 92, height: 28, radius: 14 },
  ear: { width: 300, height: 32, radius: 16 },
  line: { width: 340, height: 36, radius: 18 },
  status: { width: 300, height: 32, radius: 16 },
  card: { width: 360, minHeight: 96, maxHeight: 320, radius: 20 },
};

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

export interface PostureInput {
  state: string;
  /** A response card is on stage (answer, approval, or error inside a card). */
  cardOpen: boolean;
  /** Juno holds the physical cursor; the island has to say so in words. */
  driving?: boolean;
}

/** Which posture a combination of backend state and local state gets. */
export function posture({ state, cardOpen, driving = false }: PostureInput): Posture {
  if (cardOpen) return "card";
  if (driving) return "status";
  if (isInputState(state)) return "line";
  if (isVoiceState(state)) return "ear";
  if (isIdleState(state)) return "capsule";
  // Working, speaking, error, success, finishing, stopping: one line.
  return "status";
}

/** Clamp a measured card content height to the card's range, in steps. */
export function cardHeightFor(contentHeight: number): number {
  const { minHeight, maxHeight } = ISLAND_SIZES.card;
  const stepped = Math.ceil(contentHeight / CARD_HEIGHT_STEP) * CARD_HEIGHT_STEP;
  return Math.min(maxHeight, Math.max(minHeight, stepped));
}

/** The island's own size for a posture. */
export function islandSize(p: Posture, cardHeight = ISLAND_SIZES.card.minHeight): IslandSize {
  if (p === "card") {
    return {
      width: ISLAND_SIZES.card.width,
      height: cardHeightFor(cardHeight),
      radius: ISLAND_SIZES.card.radius,
    };
  }
  return { ...ISLAND_SIZES[p] };
}

/** The window that holds an island of that size, plus room below it. */
export function windowSize(
  size: IslandSize,
  extraBelow = 0,
): { width: number; height: number } {
  return {
    width: size.width + 2 * SHADOW_PAD,
    height: size.height + 2 * SHADOW_PAD + Math.max(0, extraBelow),
  };
}

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

export type WordsTone = "live" | "dim" | "provisional" | "error" | "plain";

export interface Words {
  text: string;
  tone: WordsTone;
}

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
 * The one line inside the ear and status postures. Null means the posture
 * shows the dot alone. Sentence case, like every other Juno surface.
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

export interface RingInput {
  cardOpen: boolean;
  /** The answer is still arriving. */
  streaming: boolean;
  /** Rust is still working (a later step may follow the first answer). */
  working: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
}

/** Whether the linger ring should be counting down at all. */
export function ringShouldRun({ cardOpen, streaming, working, approvalPending }: RingInput): boolean {
  return cardOpen && !streaming && !working && !approvalPending;
}

// === THE CURRENT TURN ===


export interface Turn {
  /** What the person asked. Empty when the conversation has no user message. */
  question: string;
  /** The reply to it, once any of it exists (visible content or spoken text). */
  answer: ChatMessage | null;
  /** A tool waiting on Allow or Don't. */
  approval: ChatMessage | null;
  /** A tool running right now, described for the status line. */
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
