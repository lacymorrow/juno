/**
 * Type definitions for Floating Bar component
 * Legacy types for backward compatibility with the standardized UI API
 */

// === Bar State Types ===

export type BarState =
    | "default"
    | "expanding"
    | "input"
    | "shrinking"
    | "submitting"
    | "loading"
    | "finishing"
    | "success"
    | "listening"
    | "error"
    | "transcribing"
    | "speaking"
    | "dictating"
    | "always-listening"
    | "agent_responding"
    | "dictation_ready";

// === Configuration Types ===

export interface FloatingBarConfig {
    showVoiceIndicator: boolean;
    enableAnimations: boolean;
    autoHide: boolean;
    autoHideDelay: number;
    opacity: number;
}

// === Window Configuration from Tauri ===

export interface WindowConfig {
    label: string;
    title?: string;
    url?: string;
    width: number;
    height: number;
    minWidth?: number;
    minHeight?: number;
    decorations: boolean;
    transparent: boolean;
    alwaysOnTop: boolean;
    resizable: boolean;
    skipTaskbar: boolean;
    visible: boolean;
    shadow: boolean;
    center?: boolean;
    fullscreen?: boolean;
    hiddenTitle?: boolean;
}

// === Dimension Constants ===

export const FLOATING_BAR_DIMENSIONS = {
    DEFAULT_WIDTH: 110,
    DEFAULT_HEIGHT: 60,
    EXPANDED_WIDTH: 300,
    EXPANDED_HEIGHT: 100,
    MIN_WIDTH: 100,
    MIN_HEIGHT: 50,
    MAX_WIDTH: 400,
    MAX_HEIGHT: 120,
} as const;

// === Voice Mode Types ===

export type VoiceMode = "idle" | "agent" | "dictation";

export type AgentState = "idle" | "working" | "responding" | "finished" | "failed" | "cancelled" | "offline";

// === Event Types ===

export interface BarStateUpdate {
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
    voiceMode: VoiceMode;
    agentState: AgentState | null;
}

// === Animation Constants ===

export const ANIMATION_DURATIONS = {
    EXPAND: 300,
    SHRINK: 300,
    FADE: 200,
    SLIDE: 250,
} as const;

// === CSS Classes ===

export const BAR_STYLES = {
    container: {
        default: "backdrop-blur-xl bg-black/20 border border-white/20",
        expanded: "backdrop-blur-xl bg-black/30 border border-white/30",
        error: "backdrop-blur-xl bg-red-500/20 border border-red-500/40",
        success: "backdrop-blur-xl bg-emerald-500/20 border border-emerald-500/40",
    },
    animation: {
        expand: "transition-all duration-300 ease-out",
        shrink: "transition-all duration-300 ease-in",
        fade: "transition-opacity duration-200",
    },
} as const;
