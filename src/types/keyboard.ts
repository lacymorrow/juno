// Types for keyboard shortcuts configuration

export interface KeyboardShortcuts {
    agent_mode: string;        // Default: Alt+D (Option+D on macOS)
    dictation_input: string;   // Default: Alt+Space (Option+Space on macOS)
    stop_current_task: string; // Default: Escape
    open_settings: string;     // Default: Cmd+, (Ctrl+, on non-macOS)
    voice_activation: string;  // Default: Option+Shift+V — always-on global voice shortcut
}

// Types for agent trigger mode configuration
export type AgentTriggerMode = "tap" | "hold";

export interface AgentTriggerModeConfig {
    mode: AgentTriggerMode;
    description: string;
}

export interface ShortcutInputProps {
    label: string;
    description: string;
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    disabled?: boolean;
}

export interface ShortcutValidationResult {
    isValid: boolean;
    error?: string;
}

export interface ShortcutConflict {
    shortcut: string;
    conflictsWith: string[];
}
