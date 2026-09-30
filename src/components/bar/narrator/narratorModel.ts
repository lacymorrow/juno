import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

/**
 * The Bar's model: pure functions from what Rust says (the bar state) and
 * what the conversation holds (the current turn) to what is on the strip.
 * Nothing here touches the DOM or Tauri, so every row of the state table in
 * docs/plans/bar-appearance.md is a unit test.
 *
 * The Bar is the narrator. One wide strip; the turn reads left to right as a
 * timeline: what you said, a bead for each step Juno takes, then the answer.
 * A rail under the line fills as the turn goes and drains once it is done.
 */

// === SIZES ===

/** The strip is always this wide. It never changes width, so nothing on it
 *  can jump sideways when the window resizes: the window only ever changes
 *  height, and it is anchored at the top. */
export const BAR_WIDTH = 520;
export const BAR_HEIGHT = 36;
export const BAR_RADIUS = 10;

/** Empty space around the strip so its shadow is not clipped by the window. */
export const SHADOW_PAD = 16;

/** The sheet below the strip follows its content within this range. */
export const SHEET_MIN_HEIGHT = 56;
export const SHEET_MAX_HEIGHT = 320;
/** Sheet height follows content in these steps, so a streaming answer does
 *  not resize the window on every glyph. */
export const SHEET_HEIGHT_STEP = 8;

/** How long a finished answer stays before the rail has drained. */
export const DRAIN_MS = 12_000;

/** A shrinking window waits for the sheet's slide before it snaps. */
export const SHRINK_DELAY_MS = 350;

/** macOS system blue, the only accent the Bar uses. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";

export function sheetHeightFor(contentHeight: number): number {
  const stepped = Math.ceil(contentHeight / SHEET_HEIGHT_STEP) * SHEET_HEIGHT_STEP;
  return Math.min(SHEET_MAX_HEIGHT, Math.max(SHEET_MIN_HEIGHT, stepped));
}

/** The window that holds the strip, plus the sheet when one is open. */
export function windowSize(sheetHeight = 0, extraBelow = 0): { width: number; height: number } {
  return {
    width: BAR_WIDTH + 2 * SHADOW_PAD,
    height: BAR_HEIGHT + 2 * SHADOW_PAD + Math.max(0, sheetHeight) + Math.max(0, extraBelow),
  };
}

// === STATES ===

const INPUT_STATES: readonly string[] = [UI.BAR_STATES_INPUT, UI.BAR_STATES_EXPANDING];

/** The person is speaking and the line is theirs. Always listening is not
 *  here: it is a resting state that happens to listen, and the dot says so. */
const VOICE_STATES: readonly string[] = [UI.BAR_STATES_LISTENING, UI.BAR_STATES_DICTATING];

const IDLE_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
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

export const isVoiceState = (state: string): boolean => VOICE_STATES.includes(state);
export const isInputState = (state: string): boolean => INPUT_STATES.includes(state);
export const isIdleState = (state: string): boolean => IDLE_STATES.includes(state);
export const isWorkingState = (state: string): boolean => WORKING_STATES.includes(state);

// === POSTURE ===

/**
 * What the line is doing. The sheet is not a posture: it is a layer below the
 * strip that any posture but `listen` can have open.
 *
 * - rest: the dot and the last turn, dimmed, or "Ask Juno" when there is none
 *   (always listening rests too; the dot and a note say it is listening)
 * - compose: the question slot is a text field
 * - listen: your words run along the line as you speak
 * - turn: the timeline of the current turn (working, answering, done, failed)
 */
export type Posture = "rest" | "compose" | "listen" | "turn";

export interface PostureInput {
  state: string;
  /** Juno holds the physical cursor; the line has to say so. */
  driving?: boolean;
}

export function posture({ state, driving = false }: PostureInput): Posture {
  if (driving) return "turn";
  if (isInputState(state)) return "compose";
  if (isVoiceState(state)) return "listen";
  if (isIdleState(state)) return "rest";
  return "turn";
}

// === THE DOT ===

export type DotMotion = "still" | "slow" | "breathe" | "orbit" | "ripple" | "shake" | "flash";

export interface DotLook {
  color: string;
  opacity: number;
  motion: DotMotion;
}

export type Theme = "light" | "dark";

/** Ink on the strip: what text and the resting dot are drawn in. */
export function inkFor(theme: Theme): string {
  return theme === "dark" ? "#FFFFFF" : "#1C1C1E";
}

/** The dot for a bar state. One dot; no icons. */
export function dotFor(state: string, theme: Theme, driving = false): DotLook {
  const ink = inkFor(theme);
  if (driving) return { color: SYSTEM_BLUE, opacity: 1, motion: "orbit" };
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
      return { color: ink, opacity: 0.4, motion: "slow" };
    case UI.BAR_STATES_DICTATION_READY:
      return { color: SYSTEM_GREEN, opacity: 0.7, motion: "still" };
    case UI.BAR_STATES_SHRINKING:
      return { color: ink, opacity: 0.25, motion: "still" };
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_INPUT:
      return { color: ink, opacity: 0.7, motion: "still" };
    case UI.BAR_STATES_LISTENING:
      return { color: SYSTEM_BLUE, opacity: 1, motion: "breathe" };
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_TRANSCRIBING:
      return { color: SYSTEM_GREEN, opacity: 1, motion: "breathe" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { color: SYSTEM_BLUE, opacity: 0.45, motion: "slow" };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { color: SYSTEM_BLUE, opacity: 1, motion: "orbit" };
    case UI.BAR_STATES_STOPPING:
      return { color: ink, opacity: 0.5, motion: "orbit" };
    case UI.BAR_STATES_SPEAKING:
      return { color: SYSTEM_BLUE, opacity: 1, motion: "ripple" };
    case UI.BAR_STATES_ERROR:
      return { color: SYSTEM_RED, opacity: 1, motion: "shake" };
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_FINISHING:
      return { color: SYSTEM_GREEN, opacity: 1, motion: "flash" };
    default:
      return { color: ink, opacity: 0.35, motion: "still" };
  }
}

// === THE CURRENT TURN ===

export type StepStatus = "running" | "done" | "failed" | "pending" | "denied";

export interface Step {
  /** What Juno is doing, in the tool's own words. */
  description: string;
  status: StepStatus;
  toolId: string | null;
  /** The message behind the step, for the approval buttons. */
  message: ChatMessage;
}

export interface Turn {
  /** What the person asked. Empty when the conversation has no user message. */
  question: string;
  /** Every tool call since the question, in order. */
  steps: Step[];
  /** The reply, once any of it exists (visible content or spoken text). */
  answer: ChatMessage | null;
}

function stepStatus(m: ChatMessage): StepStatus {
  // A result folded into a pending row settles it, whatever the row says.
  if (m.approval_state === "pending" && m.success === undefined) return "pending";
  if (m.approval_state === "denied") return "denied";
  if (m.success === undefined) return "running";
  return m.success ? "done" : "failed";
}

/**
 * The Bar shows the current turn, not the history: the last question and
 * everything that has happened since it, as a timeline.
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
  const steps: Step[] = [];
  let answer: ChatMessage | null = null;
  for (let i = start + 1; i < messages.length; i += 1) {
    const m = messages[i];
    if (m.role === "assistant" && !m.notice) {
      const hasContent = m.content.trim().length > 0 || !!m.tts_metadata?.has_spoken_content;
      if (hasContent) answer = m;
    } else if (m.role === "tool_call_request") {
      steps.push({
        description: (m.content || m.tool_name || "a step").trim().replace(/\.$/, ""),
        status: stepStatus(m),
        toolId: m.tool_id ?? null,
        message: m,
      });
    }
  }
  return { question, steps, answer };
}

/** A stable identity for an answer, so a new one can be told from growth of the old. */
export function answerKey(turn: Turn): string {
  const a = turn.answer;
  if (!a) return "";
  return a.messageId ?? String(a.timestamp ?? "");
}

/** The step waiting on Allow or Don't, if any. */
export function pendingStep(turn: Turn): Step | null {
  return turn.steps.find((s) => s.status === "pending") ?? null;
}

/** The step running right now, if any. */
export function runningStep(turn: Turn): Step | null {
  for (let i = turn.steps.length - 1; i >= 0; i -= 1) {
    if (turn.steps[i].status === "running") return turn.steps[i];
  }
  return null;
}

/** The step that failed, if any. The rail turns red at its bead. */
export function failedStep(turn: Turn): Step | null {
  for (let i = turn.steps.length - 1; i >= 0; i -= 1) {
    if (turn.steps[i].status === "failed") return turn.steps[i];
  }
  return null;
}

// === THE ANSWER LINE ===

/**
 * The first sentence of an answer, for the one line on the strip. Markdown
 * marks and component tags are dropped: the line is prose, the sheet has the
 * rest.
 */
export function firstSentence(text: string): string {
  const prose = text
    .replace(/<[A-Za-z][^>]*\/>/g, " ")
    .replace(/<[A-Za-z][^>]*>[\s\S]*?<\/[A-Za-z]+>/g, " ")
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[#*_`>]+/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
  if (!prose) return "";
  const match = prose.match(/^(.+?[.!?])(?:\s|$)/);
  return match ? match[1] : prose;
}

/** Whether the answer holds a component (JSX), which only the sheet can draw. */
export function hasComponent(text: string): boolean {
  return /<[A-Z][A-Za-z0-9]*[\s/>]/.test(text);
}

export interface SheetInput {
  visibleText: string;
  /** The answer is still arriving. */
  streaming: boolean;
}

/**
 * The sheet opens on its own only when the line cannot hold the answer: more
 * than one sentence, or a component. A one-line answer is the line.
 */
export function sheetWanted({ visibleText, streaming }: SheetInput): boolean {
  const text = visibleText.trim();
  if (!text) return false;
  if (hasComponent(text)) return true;
  const line = firstSentence(text);
  if (!line) return streaming;
  return text.replace(/\s+/g, " ").length > line.length + 1;
}

// === THE LINE ===

export type SegmentKind = "question" | "step" | "approval" | "answer" | "error" | "driving" | "note";
export type Tone = "live" | "plain" | "dim" | "provisional" | "green" | "error";

export interface Segment {
  kind: SegmentKind;
  text: string;
  tone: Tone;
  /** A bead sits before this segment on the line. */
  bead: boolean;
  /** The bead is the live one: it pulses, and the rail reaches it. */
  live?: boolean;
  /** The bead failed: it is red. */
  failed?: boolean;
  /** Only the bead shows; the text is its tooltip. Older steps collapse so
   *  the live one always has room. */
  collapsed?: boolean;
  /** The step behind an approval segment, for its buttons. */
  step?: Step;
}

export interface LineInput {
  state: string;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  spokenText: string;
  currentError: string | null;
  /** What the person asked, from the conversation or the bar. */
  question: string;
  turn: Turn;
  /** Juno is driving the cursor; the label to show. */
  drivingLabel?: string | null;
  /** The first sentence of the answer, once there is one. */
  answerLine: string;
  /** The answer has only spoken text, no visible content. */
  spokenOnly?: boolean;
}

/** What the line says while Rust works and there is no step to point at. */
const NOTES: Record<string, string> = {
  [UI.BAR_STATES_SUBMITTING]: "Sending",
  [UI.BAR_STATES_LOADING]: "Thinking",
  [UI.BAR_STATES_AGENT_RESPONDING]: "Thinking",
  [UI.BAR_STATES_FINISHING]: "Done",
  [UI.BAR_STATES_SUCCESS]: "Done",
};

/**
 * What is on the line, left to right. Sentence case, like every other Juno
 * surface. The question slot is always first; beads and the live segment
 * follow. A `compose` posture draws a text field in the question slot itself,
 * so the caller drops the question segment there.
 */
export function lineFor(input: LineInput): Segment[] {
  const { state, turn } = input;

  // Listening: your words run along the line, nothing else.
  if (state === UI.BAR_STATES_LISTENING || state === UI.BAR_STATES_DICTATING) {
    const words = input.transcriptionText;
    if (words) {
      const tone: Tone = input.transcriptionProvisional ? "provisional" : "green";
      return [{ kind: "question", text: words, tone, bead: false }];
    }
    const text = state === UI.BAR_STATES_LISTENING ? "Listening" : "Dictating";
    return [{ kind: "note", text, tone: "dim", bead: false }];
  }
  if (state === UI.BAR_STATES_ALWAYS_LISTENING && !input.question.trim() && !input.answerLine && !input.spokenOnly) {
    return [{ kind: "note", text: "Listening for “hey Juno”", tone: "dim", bead: false }];
  }
  if (state === UI.BAR_STATES_TRANSCRIBING) {
    return [{ kind: "question", text: input.transcriptionText || "Transcribing", tone: "dim", bead: false }];
  }
  if (state === UI.BAR_STATES_STOPPING) {
    return [{ kind: "note", text: "Stopping", tone: "dim", bead: false }];
  }

  const segments: Segment[] = [];
  const question = input.question.trim();
  const isError = state === UI.BAR_STATES_ERROR;
  const hasAnswer = input.answerLine.length > 0 || !!input.spokenOnly;
  const busy =
    isWorkingState(state) || !!pendingStep(turn) || !!runningStep(turn) || !!input.drivingLabel;
  const settled = !busy && !isError;

  if (question) {
    segments.push({ kind: "question", text: question, tone: settled ? "dim" : "plain", bead: false });
  }

  // Beads: every step so far. The live one keeps its words. A done step keeps
  // them only while it is the newest thing on the line, so the line never goes
  // silent mid-turn; once something newer arrives it collapses to its bead.
  const failure = failedStep(turn);
  const last = turn.steps.length - 1;
  turn.steps.forEach((step, i) => {
    if (step.status === "pending") {
      segments.push({
        kind: "approval",
        text: `Juno wants to ${step.description}`,
        tone: "plain",
        bead: true,
        live: true,
        step,
      });
      return;
    }
    const newest = i === last && !hasAnswer && !input.drivingLabel;
    if (step.status === "failed") {
      const isTheFailure = step === failure && (isError || newest);
      segments.push({
        kind: "step",
        text: step.description,
        tone: isTheFailure ? "error" : "dim",
        bead: true,
        failed: step === failure,
        collapsed: !isTheFailure,
      });
      return;
    }
    const running = step.status === "running";
    segments.push({
      kind: "step",
      text: step.description,
      tone: running ? "live" : step.status === "denied" ? "dim" : "plain",
      bead: true,
      live: running,
      collapsed: !running && !newest,
    });
  });

  if (input.drivingLabel) {
    segments.push({ kind: "driving", text: input.drivingLabel, tone: "live", bead: true, live: true });
    return segments;
  }

  if (isError) {
    const message = input.currentError || "Something went wrong";
    const redBead = segments.find((s) => s.failed && !s.collapsed);
    if (redBead) {
      // The bead's words are the tool's; the message says why it failed.
      redBead.text = `${redBead.text}: ${message}`;
    } else {
      segments.push({ kind: "error", text: message, tone: "error", bead: segments.length > 0, failed: true });
    }
    return segments;
  }

  if (hasAnswer) {
    segments.push({
      kind: "answer",
      text: input.answerLine || input.spokenText,
      tone: settled ? (input.spokenOnly ? "dim" : "plain") : "live",
      bead: true,
      live: state === UI.BAR_STATES_AGENT_RESPONDING,
    });
    return segments;
  }

  // Nothing to narrate yet: say what Rust is doing, once, in the answer's place.
  const said = segments.some((s) => s.bead && !s.collapsed);
  if (!said) {
    const note = NOTES[state];
    if (note) {
      segments.push({ kind: "note", text: note, tone: "dim", bead: segments.length > 0, live: note !== "Done" });
    } else if (state === UI.BAR_STATES_SPEAKING && input.spokenText) {
      segments.push({ kind: "answer", text: input.spokenText, tone: "dim", bead: segments.length > 0 });
    }
  }
  return segments;
}

// === THE RAIL ===

/**
 * The rail under the line. It is the turn's progress bar while Juno works
 * and the mic meter while you speak, told apart by colour.
 *
 * - meter: green, its length is your voice level
 * - toBead: blue, reaches the bead of segment `index` (measured on screen)
 * - streaming: blue, past the last bead by `fraction` of the room that is left
 * - full: blue, the whole rail; the answer is complete
 * - drain: blue, emptying from the right; `progress` 1 is full, 0 is empty
 * - failed: red up to the bead of segment `index`, or `fraction` of the rail
 * - empty: nothing
 */
export type Rail =
  | { kind: "meter"; level: number }
  | { kind: "toBead"; index: number }
  | { kind: "streaming"; fraction: number }
  | { kind: "full" }
  | { kind: "drain"; progress: number }
  | { kind: "failed"; index: number | null; fraction: number }
  | { kind: "empty" };

export interface RailInput {
  state: string;
  audioLevel: number;
  segments: Segment[];
  /** Characters of the answer so far, for the streaming curve. */
  answerLength: number;
  streaming: boolean;
  /** Drain progress, 1 to 0, while the drain runs; null otherwise. */
  drain: number | null;
}

/** Position of the live bead in the line, if any. */
function liveBeadIndex(segments: Segment[]): number | null {
  for (let i = segments.length - 1; i >= 0; i -= 1) {
    if (segments[i].bead && segments[i].live) return i;
  }
  return null;
}

/** How far a streaming answer has filled the room past its bead. Asymptotic:
 *  never quite full until the stream ends. */
export function streamingFraction(chars: number): number {
  return 1 - Math.exp(-chars / 240);
}

export function railFor(input: RailInput): Rail {
  const { state, segments } = input;
  if (state === UI.BAR_STATES_LISTENING || state === UI.BAR_STATES_DICTATING) {
    return { kind: "meter", level: Math.max(0, Math.min(1, input.audioLevel)) };
  }
  if (state === UI.BAR_STATES_ERROR) {
    const at = segments.findIndex((s) => s.failed);
    return { kind: "failed", index: at >= 0 ? at : null, fraction: 0.5 };
  }
  if (input.drain !== null) return { kind: "drain", progress: input.drain };
  if (isIdleState(state) || isInputState(state) || state === UI.BAR_STATES_STOPPING) {
    return { kind: "empty" };
  }
  if (segments.some((s) => s.kind === "answer")) {
    if (input.streaming) return { kind: "streaming", fraction: streamingFraction(input.answerLength) };
    return { kind: "full" };
  }
  if (
    state === UI.BAR_STATES_FINISHING ||
    state === UI.BAR_STATES_SUCCESS ||
    state === UI.BAR_STATES_SPEAKING
  ) {
    return { kind: "full" };
  }
  const live = liveBeadIndex(segments);
  if (live !== null) return { kind: "toBead", index: live };
  // Transcribing, submitting, loading with nothing to point at yet: a first
  // sliver, so the person sees the turn has begun.
  return { kind: "streaming", fraction: 0 };
}

export interface DrainInput {
  /** There is an answer on the line. */
  hasAnswer: boolean;
  /** The answer is still arriving. */
  streaming: boolean;
  /** Rust is still working, or speaking the answer. */
  working: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
}

/** Whether the rail should be draining (the linger timer) at all. */
export function drainShouldRun({ hasAnswer, streaming, working, approvalPending }: DrainInput): boolean {
  return hasAnswer && !streaming && !working && !approvalPending;
}
