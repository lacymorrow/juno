/**
 * A spoken turn, from the bar's side: is the microphone open, and which
 * controls does the person get while it is.
 *
 * One mapping for every appearance. The Pill used to decide this from its own
 * list of "voice" states, and the moment live words arrived Rust moved the bar
 * to TRANSCRIBING, which the Pill files under "working": Send vanished and the
 * only control left was a stop square that reached the coordinated stop. The
 * Island and the Avatar had no controls at all while you talked. Every look
 * now asks this module, so they cannot drift apart again.
 *
 * The meanings are fixed:
 *   send   = finish now and submit what was said
 *   type   = throw the audio away and open the composer
 *   cancel = throw the audio away, send nothing
 */
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { COMMANDS, EVENTS, UI } from "@/lib/constants.generated";

export type VoiceTurnControl = "send" | "type" | "cancel";

export interface VoiceTurnState {
  barState: string;
  /** Live words while you talk. Rust sets it only for partials from an open mic. */
  transcriptionProvisional?: boolean;
  isDictationMode?: boolean;
}

/**
 * Is a turn recording: the microphone is open and what was said can still be
 * sent or thrown away.
 *
 * TRANSCRIBING means two things in Rust: live words arriving from an open mic
 * (provisional), and "the mic is closed, the final decode is running" (Rust
 * sets it with provisional false the moment a stop is claimed). Only the first
 * is still recording.
 */
export function isRecordingTurn(s: VoiceTurnState): boolean {
  if (s.barState === UI.BAR_STATES_LISTENING || s.barState === UI.BAR_STATES_DICTATING) {
    return true;
  }
  return s.barState === UI.BAR_STATES_TRANSCRIBING && s.transcriptionProvisional === true;
}

/**
 * The controls a bar shows for the current state, in order. Empty when no turn
 * is recording. A wake-phrase capture gets cancel only: the engine decides when
 * that sentence ends, so there is nothing to send and nothing to switch to.
 */
export function voiceTurnControls(s: VoiceTurnState): readonly VoiceTurnControl[] {
  if (s.barState === UI.BAR_STATES_ALWAYS_LISTENING) return ["cancel"];
  if (isRecordingTurn(s)) return ["send", "type", "cancel"];
  return [];
}

export const VOICE_TURN_LABELS: Record<VoiceTurnControl, string> = {
  send: "Send",
  type: "Type instead",
  cancel: "Cancel without sending",
};

/** Is the open session a dictation rather than a spoken query to Juno? */
export function isDictationTurn(s: VoiceTurnState): boolean {
  return Boolean(s.isDictationMode) || s.barState === UI.BAR_STATES_DICTATING;
}

/** Finish the turn and submit what was said. Rust routes it to whichever session is open. */
export async function sendVoiceTurn(): Promise<void> {
  try {
    await invoke(COMMANDS.AGENT_AGENT_VOICE, { action: "stop" });
  } catch (error) {
    console.error("voiceTurn: could not send what was said:", error);
  }
}

/**
 * Throw the turn away. A dictation is cancelled through its own event so the
 * dictation state machine unwinds with it; a spoken query through
 * `agent_voice`. Both land on the same Rust end path, which closes the
 * microphone before it returns.
 */
export async function cancelVoiceTurn(s: VoiceTurnState): Promise<void> {
  try {
    if (isDictationTurn(s)) {
      await emit(EVENTS.DICTATION_TRANSCRIPTION_CANCEL);
      return;
    }
    await invoke(COMMANDS.AGENT_AGENT_VOICE, { action: "cancel" });
  } catch (error) {
    console.error("voiceTurn: could not cancel listening:", error);
  }
}
