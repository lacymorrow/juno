/**
 * What the Pill's status dot says, as data.
 *
 * Rust owns whether Juno can reach the network and its provider
 * (`connectivity.rs`, emitted as `connectivity-changed`). The bar state says
 * whether a microphone is open. This file only turns those two facts into a
 * colour and a sentence; it decides nothing about connectivity itself.
 *
 * The colour language is deliberately small (flat macOS, one accent):
 *   neutral  = everything is fine, or Juno is busy (motion says which)
 *   blue     = a microphone is open and Juno is listening
 *   red      = Juno cannot answer: no internet, no provider, not signed in
 */

import { UI } from "@/lib/constants.generated";

export type ConnectivityStatus =
  | "connected"
  | "offline"
  | "signed_out"
  | "provider_unreachable";

/** `connectivity::Snapshot` in Rust. */
export interface Connectivity {
  status: ConnectivityStatus;
  /** A short sentence for the status, e.g. "No internet connection". */
  label: string;
  /** The provider's everyday name, e.g. "Claude". */
  provider: string;
}

export type DotTone = "neutral" | "listening" | "down";

/** macOS system blue and system red (dark appearance). */
export const DOT_COLORS: Record<Exclude<DotTone, "neutral">, string> = {
  listening: "#0A84FF",
  down: "#FF453A",
};

const LISTENING_STATES: readonly string[] = [
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_ALWAYS_LISTENING,
  UI.BAR_STATES_DICTATING,
];

const WORKING_STATES: readonly string[] = [
  UI.BAR_STATES_SUBMITTING,
  UI.BAR_STATES_LOADING,
  UI.BAR_STATES_AGENT_RESPONDING,
  UI.BAR_STATES_FINISHING,
  UI.BAR_STATES_STOPPING,
];

export function isDown(connectivity: Connectivity | null): boolean {
  return connectivity !== null && connectivity.status !== "connected";
}

/**
 * The dot's colour. An open microphone wins over a lost connection: while
 * someone is talking, the one thing the dot must say is "I hear you", and
 * speech-to-text runs on the Mac. The red comes back the moment it closes.
 */
export function dotTone(state: string, connectivity: Connectivity | null): DotTone {
  if (LISTENING_STATES.includes(state)) return "listening";
  if (isDown(connectivity)) return "down";
  return "neutral";
}

/** The dot's accessible label and tooltip: what it is showing right now. */
export function dotLabel(
  state: string,
  connectivity: Connectivity | null,
  { driving = false, voicePaused = false }: { driving?: boolean; voicePaused?: boolean } = {},
): string {
  if (driving) return "Juno is using the pointer";
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return "Listening";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "Always listening";
    case UI.BAR_STATES_DICTATING:
      return "Dictating";
    default:
      break;
  }
  if (connectivity && isDown(connectivity)) return connectivity.label;
  if (state === UI.BAR_STATES_TRANSCRIBING) return "Transcribing";
  if (WORKING_STATES.includes(state)) return "Working";
  if (state === UI.BAR_STATES_SPEAKING) return "Speaking";
  const connected = connectivity?.label ?? "Connected";
  return voicePaused ? `${connected}, wake phrase paused` : connected;
}
