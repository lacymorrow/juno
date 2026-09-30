import { UI } from "@/lib/constants.generated";

/**
 * The Avatar's model: pure functions from what Rust says (the bar state, the
 * words in flight) and what the avatar knows locally (is an answer on stage,
 * is a tool waiting, is Juno driving) to the scene: how the head looks, what
 * is in your bubble, what is in Juno's. Nothing here touches the DOM, Rive or
 * Tauri, so every row of the table in docs/plans/avatar-appearance.md is a
 * unit test.
 */

// === SIZES ===

/** The head's box. The Rive sphere fills most of it; the face sits inside. */
export const HEAD = 84;
/** Room around everything so shadows and the lean are never clipped. */
export const PAD = 16;
/** Between the head and the first bubble. */
export const GAP = 10;
/** Width of the panel once any bubble is up. Constant on purpose: the window
 *  only ever changes width between rest and open, and the head sits at its
 *  centre both times, so it never slides. */
export const PANEL_WIDTH = 356;
/** The widest a bubble may grow. Sized to content below that. */
export const BUBBLE_MAX_WIDTH = 300;
/** Your line, once Juno's bubble is up: a short quote on your side, clear
 *  of the tail that runs from Juno's bubble up to the head. */
export const QUOTE_MAX_WIDTH = 140;
/** An answer taller than this scrolls inside its bubble. */
export const ANSWER_MAX_HEIGHT = 320;
/** The stack of bubbles is measured; its height lands in these steps so a
 *  streaming answer does not resize the window on every glyph. */
export const STACK_STEP = 8;
/** How long a finished answer stays up before the avatar lets it go. */
export const LINGER_MS = 12_000;
/** A shrinking window waits for the bubble to leave before it snaps. */
export const SHRINK_DELAY_MS = 350;

/** macOS system blue: the only accent. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";

// === STATE FAMILIES ===

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

export const isInputState = (state: string): boolean => INPUT_STATES.includes(state);
export const isVoiceState = (state: string): boolean => VOICE_STATES.includes(state);
export const isIdleState = (state: string): boolean => IDLE_STATES.includes(state);
export const isWorkingState = (state: string): boolean => WORKING_STATES.includes(state);

// === THE HEAD ===

/** The inputs the Rive file actually has (state machine "default"). */
export type RiveState = "idle" | "listening" | "thinking" | "speaking";

/**
 * What the container and the face do. The Rive file has no eyes, mouth or
 * neck, so these are CSS: transforms on the head's box, and the SVG face.
 */
export type Gesture =
  | "calm" // at rest: a blink now and then
  | "lean" // listening: leans toward you, eyes on your bubble
  | "attend" // typing: eyes on your bubble, no lean
  | "think" // working: tilts away, eyes up
  | "talk" // speaking: the mouth moves
  | "nod" // finished: one nod
  | "wince" // failed: a flinch, eyes squint
  | "wide"; // waiting on you: eyes open wide

export interface Cue {
  color: string;
  opacity: number;
  motion: "breathe" | "slow" | "still";
}

export interface HeadLook {
  rive: RiveState;
  gesture: Gesture;
  /** The small light beside the head: blue while Juno listens, green while
   *  your speech becomes text. Null otherwise. */
  cue: Cue | null;
}

// === THE BUBBLES ===

export type YourTone = "live" | "provisional" | "dictation" | "dim";

export type YourBubble =
  | { kind: "words"; text: string; tone: YourTone }
  | { kind: "composer"; ready: boolean };

export type JunoBubble =
  | { kind: "thought"; text: string }
  | { kind: "speech"; text: string }
  | { kind: "answer" }
  | { kind: "question" }
  | { kind: "error"; text: string }
  /** A cursor-control notice with nothing else to say. Local, never from a state. */
  | { kind: "notice" };

export interface Scene {
  head: HeadLook;
  yours: YourBubble | null;
  junos: JunoBubble | null;
}

export interface SceneInput {
  state: string;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  spokenText: string;
  currentError: string | null;
  /** What the person asked, from the conversation or the bar. */
  question: string;
  /** A tool Juno is running right now, described. */
  runningTool?: string | null;
  /** Juno is driving the cursor; the label to show. */
  drivingLabel?: string | null;
  /** An answer bubble is on stage (opened by a reply, kept by the linger). */
  answerOpen: boolean;
  /** The reply has something to show: visible text or spoken text. */
  hasAnswerContent: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
}

const BLUE_CUE: Cue = { color: SYSTEM_BLUE, opacity: 1, motion: "breathe" };
const GREEN_CUE: Cue = { color: SYSTEM_GREEN, opacity: 1, motion: "breathe" };

export function headFor(input: SceneInput): HeadLook {
  const { state } = input;
  const driving = !!input.drivingLabel;

  let rive: RiveState = "idle";
  if (isVoiceState(state)) rive = "listening";
  else if (
    driving ||
    state === UI.BAR_STATES_TRANSCRIBING ||
    state === UI.BAR_STATES_SUBMITTING ||
    state === UI.BAR_STATES_LOADING ||
    state === UI.BAR_STATES_STOPPING
  ) {
    rive = "thinking";
  } else if (state === UI.BAR_STATES_AGENT_RESPONDING) {
    rive = input.hasAnswerContent ? "speaking" : "thinking";
  } else if (state === UI.BAR_STATES_SPEAKING) rive = "speaking";

  let gesture: Gesture = "calm";
  if (state === UI.BAR_STATES_ERROR) gesture = "wince";
  else if (input.approvalPending) gesture = "wide";
  else if (state === UI.BAR_STATES_FINISHING || state === UI.BAR_STATES_SUCCESS) gesture = "nod";
  else if (driving) gesture = "think";
  else if (state === UI.BAR_STATES_LISTENING || state === UI.BAR_STATES_DICTATING) gesture = "lean";
  else if (isInputState(state)) gesture = "attend";
  else if (rive === "thinking") gesture = "think";
  else if (rive === "speaking") gesture = "talk";

  let cue: Cue | null = null;
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      cue = BLUE_CUE;
      break;
    case UI.BAR_STATES_ALWAYS_LISTENING:
      cue = { color: SYSTEM_BLUE, opacity: 0.45, motion: "slow" };
      break;
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_TRANSCRIBING:
      cue = GREEN_CUE;
      break;
    case UI.BAR_STATES_DICTATION_READY:
      cue = { color: SYSTEM_GREEN, opacity: 0.6, motion: "still" };
      break;
    default:
      cue = null;
  }

  return { rive, gesture, cue };
}

/** The bubble on your side. Your words as you say them, or the composer. */
export function yourBubbleFor(input: SceneInput): YourBubble | null {
  const { state } = input;
  const said = input.transcriptionText.trim();
  const live: YourTone = input.transcriptionProvisional ? "provisional" : "live";
  switch (state) {
    case UI.BAR_STATES_EXPANDING:
      return { kind: "composer", ready: false };
    case UI.BAR_STATES_INPUT:
      return { kind: "composer", ready: true };
    case UI.BAR_STATES_LISTENING:
      return said ? { kind: "words", text: said, tone: live } : { kind: "words", text: "Listening", tone: "dim" };
    case UI.BAR_STATES_DICTATING:
      return said
        ? { kind: "words", text: said, tone: input.transcriptionProvisional ? "provisional" : "dictation" }
        : { kind: "words", text: "Dictating", tone: "dim" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { kind: "words", text: "Listening for “hey Juno”", tone: "dim" };
    case UI.BAR_STATES_TRANSCRIBING:
      return { kind: "words", text: said || "Transcribing", tone: "dim" };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
    case UI.BAR_STATES_STOPPING:
      return input.question ? { kind: "words", text: input.question, tone: "dim" } : null;
    default:
      // Rest. Your question stays above an answer that is still up.
      return input.answerOpen && input.question ? { kind: "words", text: input.question, tone: "dim" } : null;
  }
}

/** Juno's bubble: a thought while working, speech or the answer, a question
 *  when a tool needs you, a red-edged bubble when something failed. */
export function junoBubbleFor(input: SceneInput): JunoBubble | null {
  const { state } = input;
  if (input.approvalPending) return { kind: "question" };
  if (state === UI.BAR_STATES_ERROR) {
    return { kind: "error", text: input.currentError || "Something went wrong" };
  }
  if (input.drivingLabel) return { kind: "thought", text: `Juno is ${input.drivingLabel}` };
  if (state === UI.BAR_STATES_STOPPING) return { kind: "thought", text: "Stopping" };
  if (input.answerOpen && input.hasAnswerContent) return { kind: "answer" };
  switch (state) {
    case UI.BAR_STATES_SUBMITTING:
      return { kind: "thought", text: "Thinking" };
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { kind: "thought", text: input.runningTool || "Working" };
    case UI.BAR_STATES_SPEAKING:
      return input.spokenText ? { kind: "speech", text: input.spokenText } : null;
    default:
      return null;
  }
}

export function sceneFor(input: SceneInput): Scene {
  return { head: headFor(input), yours: yourBubbleFor(input), junos: junoBubbleFor(input) };
}

// === THE ANSWER ===

export interface AnswerParts {
  /** The spoken sentence to show above the notes, or null when it would only
   *  repeat them. */
  spoken: string | null;
  /** The visible answer (markdown, components). Empty for a spoken-only reply. */
  notes: string;
  /** Both parts are shown, so the spoken one can be folded away. */
  foldable: boolean;
}

/** Lowercase, one space between words, no trailing punctuation. */
function plain(text: string): string {
  return text
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/[\s.!?,;:\u2026]+$/u, "")
    .trim();
}

/**
 * How an answer is laid out in Juno's bubble. Juno's voice usually speaks the
 * first sentence of the visible answer, so the spoken line is shown only when
 * it says something the notes do not. A spoken-only reply is the body itself.
 */
export function answerParts(visible: string, spoken: string): AnswerParts {
  const notes = visible.trim();
  const said = spoken.trim();
  if (!said) return { spoken: null, notes, foldable: false };
  if (!notes) return { spoken: said, notes: "", foldable: false };
  const heard = plain(said);
  if (heard && plain(notes).includes(heard)) return { spoken: null, notes, foldable: false };
  return { spoken: said, notes, foldable: true };
}

// === THE WINDOW ===

export interface WindowBox {
  width: number;
  height: number;
  /** Logical y of the head's centre, measured from the edge the window keeps
   *  still: the top when bubbles hang below the head, the bottom when they
   *  rise above it. Constant either way, so the head never moves on screen. */
  anchorY: number;
}

/** The head's centre from the near edge. Same number in every posture. */
export const HEAD_ANCHOR = PAD + HEAD / 2;

/** Clamp a measured stack height to steps, so streaming does not jitter. */
export function stackHeightFor(measured: number): number {
  if (measured <= 0) return 0;
  return Math.ceil(measured / STACK_STEP) * STACK_STEP;
}

/** The window for a scene: the head alone at rest, the panel once any
 *  bubble is up. Height follows the measured bubble stack. */
export function windowFor(scene: Scene, stackHeight: number): WindowBox {
  const open = scene.yours !== null || scene.junos !== null;
  if (!open) {
    return { width: HEAD + 2 * PAD, height: HEAD + 2 * PAD, anchorY: HEAD_ANCHOR };
  }
  const stack = stackHeightFor(stackHeight);
  return {
    width: PANEL_WIDTH,
    height: PAD + HEAD + (stack > 0 ? GAP + stack : 0) + PAD,
    anchorY: HEAD_ANCHOR,
  };
}

/** Whether a finished answer's linger clock should be counting at all. */
export function lingerShouldRun(input: {
  answerOpen: boolean;
  streaming: boolean;
  working: boolean;
  approvalPending: boolean;
  composerOpen: boolean;
}): boolean {
  return (
    input.answerOpen && !input.streaming && !input.working && !input.approvalPending && !input.composerOpen
  );
}
