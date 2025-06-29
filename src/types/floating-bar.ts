// Shared types for floating bar components
export type BarState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "submitting"     // Immediate feedback when query is submitted
  | "loading"        // Universal processing state (replaces agent_listening, agent_thinking)
  | "finishing"
  | "success"
  | "error"
  | "dictation_ready"
  | "agent_responding" // Only unique agent state
  | "speaking"
  | "listening"       // Universal voice listening state
  | "transcribing"    // Universal transcription state (replaces dictation_processing)
  | "dictating"       // Universal dictation state (replaces dictation_active)
  | "always-listening";

export interface BarStateData {
  barState: BarState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  audioLevel: number;
  voiceMode: "dictation" | "agent" | "idle";
  agentState?: string | null; // "Finished", "Failed", "Cancelled", "Offline"
}

export interface FloatingBarConfig {
  showVoiceIndicator: boolean;
  enableAnimations: boolean;
  autoHide: boolean;
  autoHideDelay: number;
  opacity: number;
}

// Window configuration type
export interface WindowConfig {
  label: string;
  width?: number;
  height?: number;
}

// Constants for floating bar dimensions
export const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 110,
  DEFAULT_HEIGHT: 60,
  EXPANDED_WIDTH: 320,
  EXPANDED_HEIGHT: 80,
} as const;
