import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

/**
 * The Studio's model: pure functions from what Rust says (the bar state) and
 * what the studio knows locally (is a script open, is Juno driving, what the
 * conversation holds) to what is on screen. Nothing here touches the DOM or
 * Tauri, so every row of the state table in docs/plans/studio-appearance.md
 * is a unit test.
 */

/** The five shapes the studio takes. */
export type Posture = "deck" | "take" | "type" | "work" | "script";

/** Empty space around the studio so its shadow is not clipped by the window. */
export const SHADOW_PAD = 24;

/** How long a finished script lingers before the studio settles back. */
export const LINGER_MS = 12_000;

/** A shrinking window waits for the studio's spring before it snaps. */
export const SHRINK_DELAY_MS = 350;

/** Script height follows content in these steps, so a streaming answer does
 *  not resize the window on every glyph. */
export const SCRIPT_HEIGHT_STEP = 8;

/** How often the waveform takes a sample, and how many it keeps. */
export const WAVE_SAMPLE_MS = 50;
export const WAVE_SAMPLES = 48;

export interface StudioSize {
  width: number;
  height: number;
  radius: number;
}

/** Studio sizes per posture. Take and work match on purpose: the studio must
 *  not lurch the instant someone stops talking. */
export const STUDIO_SIZES: {
  deck: StudioSize;
  take: StudioSize;
  type: StudioSize;
  work: StudioSize;
  script: { width: number; minHeight: number; maxHeight: number; radius: number };
} = {
  deck: { width: 132, height: 30, radius: 15 },
  take: { width: 320, height: 66, radius: 16 },
  type: { width: 320, height: 40, radius: 16 },
  work: { width: 320, height: 66, radius: 16 },
  script: { width: 360, minHeight: 120, maxHeight: 340, radius: 18 },
};

const INPUT_STATES: readonly string[] = [UI.BAR_STATES_INPUT, UI.BAR_STATES_EXPANDING];

/** States in which the person is speaking, or may at any moment. Partial
 *  transcripts arrive as `transcribing`, so it belongs to the take. */
const VOICE_STATES: readonly string[] = [
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_TRANSCRIBING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

const IDLE_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
];

/** States in which Rust is busy on the person's behalf. Escape stops work. */
export const WORKING_STATES: readonly string[] = [
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
  /** A script is on stage (answer, approval, or error inside a script). */
  scriptOpen: boolean;
  /** Juno holds the physical cursor; the studio has to say so in words. */
  driving?: boolean;
}

/** Which posture a combination of backend state and local state gets. */
export function posture({ state, scriptOpen, driving = false }: PostureInput): Posture {
  if (scriptOpen) return "script";
  if (driving) return "work";
  if (isInputState(state)) return "type";
  if (isVoiceState(state)) return "take";
  if (isIdleState(state)) return "deck";
  // Working, speaking, error, success, finishing, stopping: the tape runs.
  return "work";
}

/** Clamp a measured script content height to the script's range, in steps. */
export function scriptHeightFor(contentHeight: number): number {
  const { minHeight, maxHeight } = STUDIO_SIZES.script;
  const stepped = Math.ceil(contentHeight / SCRIPT_HEIGHT_STEP) * SCRIPT_HEIGHT_STEP;
  return Math.min(maxHeight, Math.max(minHeight, stepped));
}

/** The studio's own size for a posture. */
export function studioSize(p: Posture, scriptHeight = STUDIO_SIZES.script.minHeight): StudioSize {
  if (p === "script") {
    return {
      width: STUDIO_SIZES.script.width,
      height: scriptHeightFor(scriptHeight),
      radius: STUDIO_SIZES.script.radius,
    };
  }
  return { ...STUDIO_SIZES[p] };
}

/** The window that holds a studio of that size, plus room below it. */
export function windowSize(
  size: StudioSize,
  extraBelow = 0,
): { width: number; height: number } {
  return {
    width: size.width + 2 * SHADOW_PAD,
    height: size.height + 2 * SHADOW_PAD + Math.max(0, extraBelow),
  };
}

// === THE WAVEFORM ===

/** macOS system blue: Juno. */
export const SYSTEM_BLUE = "#0A84FF";
/** System green: your speech becoming text. */
export const SYSTEM_GREEN = "#30D158";
/** System red: something failed. */
export const SYSTEM_RED = "#FF453A";
/** The hairline at rest. */
export const HAIRLINE = "rgba(0,0,0,0.18)";

/**
 * What the strip draws.
 * live: follows audioLevel. speech: follows the synthesised envelope of the
 * sentence Juno is speaking. drift: a blue baseline that moves by a pixel
 * (the mic is open). settle: the last samples decay to flat. shake: one
 * shake, then flat. flat: the hairline.
 */
export type WaveMode = "flat" | "live" | "speech" | "drift" | "settle" | "shake";

export interface WaveLook {
  color: string;
  opacity: number;
  mode: WaveMode;
}

export interface WaveInput {
  state: string;
  driving?: boolean;
  /** Rust is speaking this sentence aloud right now. */
  spokenText?: string;
}

/** The waveform for a bar state. Colour is who is speaking; motion is whether anyone is. */
export function waveFor({ state, driving = false, spokenText = "" }: WaveInput): WaveLook {
  if (driving) return { color: HAIRLINE, opacity: 1, mode: "flat" };
  switch (state) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_TRANSCRIBING:
      return { color: SYSTEM_GREEN, opacity: 1, mode: "live" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { color: SYSTEM_BLUE, opacity: 0.7, mode: "drift" };
    case UI.BAR_STATES_DICTATION_READY:
      return { color: SYSTEM_GREEN, opacity: 0.7, mode: "flat" };
    case UI.BAR_STATES_SUBMITTING:
      return { color: SYSTEM_GREEN, opacity: 1, mode: "settle" };
    case UI.BAR_STATES_SPEAKING:
      return spokenText
        ? { color: SYSTEM_BLUE, opacity: 1, mode: "speech" }
        : { color: SYSTEM_BLUE, opacity: 0.7, mode: "flat" };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return { color: SYSTEM_GREEN, opacity: 1, mode: "settle" };
    case UI.BAR_STATES_ERROR:
      return { color: SYSTEM_RED, opacity: 1, mode: "shake" };
    case UI.BAR_STATES_SHRINKING:
      return { color: HAIRLINE, opacity: 0.4, mode: "flat" };
    case UI.BAR_STATES_STOPPING:
      return { color: HAIRLINE, opacity: 0.5, mode: "flat" };
    default:
      return { color: HAIRLINE, opacity: 1, mode: "flat" };
  }
}

/**
 * The next smoothed level: a fast attack so the first syllable shows at once,
 * a slow release so a pause between words does not cut the strip to zero.
 */
export function smoothLevel(prev: number, target: number, dtMs: number): number {
  const t = Math.max(0, Math.min(1, target));
  const tau = t > prev ? 40 : 160;
  const k = 1 - Math.exp(-Math.max(0, dtMs) / tau);
  return prev + (t - prev) * k;
}

function hashText(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i += 1) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

/**
 * A level for the sentence Juno is speaking, at a moment. No level exists
 * for TTS, so this is honest about being a cadence, not a measurement:
 * syllables at about four a second, a dip between words, and a ripple seeded
 * by the text so two sentences never move the same way. Always in 0..1 and
 * never zero while there is text, so the strip reads as a voice, not a fault.
 */
export function speechEnvelope(text: string, tMs: number): number {
  if (!text.trim()) return 0;
  const seed = hashText(text) % 997;
  const t = Math.max(0, tMs) / 1000;
  const syllable = Math.abs(Math.sin(Math.PI * 4 * t + seed));
  const word = 0.75 + 0.25 * Math.cos(2 * Math.PI * 1.3 * t + seed * 0.01);
  const ripple = 0.08 * Math.sin(2 * Math.PI * 7.1 * t + seed * 0.1);
  const level = 0.18 + 0.62 * syllable * word + ripple;
  return Math.max(0.12, Math.min(1, level));
}

// === THE LINE ===

export type LineTone = "live" | "dim" | "provisional" | "error" | "plain" | "speech" | "direction";

export interface Line {
  text: string;
  tone: LineTone;
}

export interface LineInput {
  state: string;
  transcriptionText: string;
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
 * The line beneath the strip. In the take it is the placeholder until the
 * first partial arrives (the teleprompter takes over). In work it is the one
 * line. Null means the posture shows the strip alone.
 */
export function lineFor(input: LineInput): Line | null {
  const { state } = input;
  if (input.drivingLabel) return { text: `Juno is ${input.drivingLabel}`, tone: "direction" };
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return input.transcriptionText ? null : { text: "Listening", tone: "dim" };
    case UI.BAR_STATES_DICTATING:
      return input.transcriptionText ? null : { text: "Dictating", tone: "dim" };
    case UI.BAR_STATES_TRANSCRIBING:
      return input.transcriptionText ? null : { text: "Transcribing", tone: "dim" };
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { text: "Mic open", tone: "dim" };
    case UI.BAR_STATES_SUBMITTING:
      return { text: input.question || "Sending", tone: "dim" };
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      if (input.runningTool) return { text: `Juno runs ${input.runningTool}`, tone: "direction" };
      return { text: input.question || "Working", tone: "dim" };
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
      return { text: "Done", tone: "plain" };
    case UI.BAR_STATES_SPEAKING:
      return input.spokenText ? { text: input.spokenText, tone: "speech" } : { text: "Speaking", tone: "dim" };
    case UI.BAR_STATES_ERROR:
      return { text: input.currentError || "Something went wrong", tone: "error" };
    case UI.BAR_STATES_STOPPING:
      return { text: "Stopping", tone: "dim" };
    default:
      return null;
  }
}

// === THE TELEPROMPTER ===

export interface TeleprompterText {
  /** Words the engine has committed. Solid. */
  final: string;
  /** Words that may still change. Lighter. */
  provisional: string;
}

/**
 * What the engine has committed so far. Rust sends the whole text each time
 * with a flag saying whether its tail is provisional; the studio keeps the
 * last committed text so it can draw the split.
 */
export function commitTranscript(committed: string, text: string, provisional: boolean): string {
  if (!text) return "";
  if (!provisional) return text;
  return text.startsWith(committed) ? committed : "";
}

/** Split the current text into its solid and lighter parts. */
export function teleprompterText(text: string, provisional: boolean, committed: string): TeleprompterText {
  if (!text) return { final: "", provisional: "" };
  if (!provisional) return { final: text, provisional: "" };
  if (committed && text.startsWith(committed)) {
    return { final: committed, provisional: text.slice(committed.length) };
  }
  return { final: "", provisional: text };
}

// === THE TAPE ===

export type CounterMode = "off" | "running" | "held";

export interface CounterInput {
  state: string;
  /** The conversation is still processing (a later step may follow). */
  processing: boolean;
  scriptOpen: boolean;
}

/**
 * Whether the tape counter counts, holds, or is absent. It counts while Rust
 * or the conversation is working; it holds once they stop so the number
 * stays honest; it is absent while nothing has been asked.
 */
export function counterFor({ state, processing, scriptOpen }: CounterInput): CounterMode {
  const running =
    state === UI.BAR_STATES_SUBMITTING ||
    state === UI.BAR_STATES_LOADING ||
    state === UI.BAR_STATES_AGENT_RESPONDING ||
    processing;
  if (running) return "running";
  if (scriptOpen) return "held";
  switch (state) {
    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
    case UI.BAR_STATES_STOPPING:
      return "held";
    default:
      return "off";
  }
}

/** `mm:ss.f`, the way a tape counter reads. */
export function formatCounter(ms: number): string {
  const total = Math.max(0, ms);
  const minutes = Math.floor(total / 60_000);
  const seconds = Math.floor((total % 60_000) / 1000);
  const tenths = Math.floor((total % 1000) / 100);
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  return `${mm}:${ss}.${tenths}`;
}

export interface TapeInput {
  scriptOpen: boolean;
  /** The answer is still arriving. */
  streaming: boolean;
  /** Rust is still working (a later step may follow the first answer). */
  working: boolean;
  /** A tool is waiting on Allow or Don't. */
  approvalPending: boolean;
}

/** Whether the tape under the script header should be running out. */
export function tapeShouldRun({ scriptOpen, streaming, working, approvalPending }: TapeInput): boolean {
  return scriptOpen && !streaming && !working && !approvalPending;
}

// === THE SCRIPT ===

export type DirectionKind = "running" | "done" | "approval";

export interface Direction {
  kind: DirectionKind;
  /** The tool's description, as Rust sent it. */
  text: string;
  message: ChatMessage;
}

export interface Script {
  /** What the person asked. Empty when the conversation has no user message. */
  question: string;
  /** The reply, once any of it exists (visible content or spoken text). */
  answer: ChatMessage | null;
  /** A tool waiting on Allow or Don't. */
  approval: ChatMessage | null;
  /** Every tool call since the question, in order, as stage directions. */
  directions: Direction[];
  /** A tool running right now, described for the work line. */
  runningTool: string | null;
}

function toolText(m: ChatMessage): string {
  return (m.content || m.tool_name || "a tool").trim().replace(/\.$/, "");
}

/**
 * The studio shows the current take, not the history: the last question and
 * everything that has happened since it.
 */
export function scriptFor(messages: ChatMessage[]): Script {
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
  const directions: Direction[] = [];
  for (let i = start + 1; i < messages.length; i += 1) {
    const m = messages[i];
    if (m.role === "assistant" && !m.notice) {
      const hasContent = m.content.trim().length > 0 || !!m.tts_metadata?.has_spoken_content;
      if (hasContent) answer = m;
    } else if (m.role === "tool_call_request") {
      const text = toolText(m);
      if (m.approval_state === "pending") {
        approval = m;
        directions.push({ kind: "approval", text, message: m });
      } else if (m.success === undefined && m.approval_state !== "denied") {
        runningTool = text;
        directions.push({ kind: "running", text, message: m });
      } else {
        runningTool = null;
        directions.push({ kind: "done", text, message: m });
      }
    }
  }
  return { question, answer, approval, directions, runningTool };
}

/** A stable identity for an answer, so a new one can be told from growth of the old. */
export function answerKey(script: Script): string {
  const a = script.answer;
  if (!a) return "";
  return a.messageId ?? String(a.timestamp ?? "");
}

/** The two channels of a reply: what was said aloud and what was shown. */
export function replyChannels(answer: ChatMessage | null): { spoken: string; visible: string } {
  const visible = answer?.content.trim() ?? "";
  const spoken =
    answer?.tts_metadata?.total_spoken_text?.trim() ||
    answer?.tts_metadata?.tts_parts?.map((p) => p.trim()).join(" ") ||
    "";
  return { spoken, visible };
}
