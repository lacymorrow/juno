// Shared types for floating bar components
export type BarState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "loading"
  | "finishing"
  | "success"
  | "error"
  | "dictation_ready"
  | "dictation_active"
  | "dictation_processing"
  | "agent_listening"
  | "agent_thinking"
  | "agent_responding"
  | "speaking"
  | "listening"
  | "transcribing"
  | "dictating"
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
