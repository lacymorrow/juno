// Types for keyboard shortcuts configuration

// Read-only. Bindings are configured on the Triggers screen; these two are
// fixed constants in Rust and are exposed only so screens can display them.
export interface KeyboardShortcuts {
    agent_mode: string;        // Derived from the agent trigger
    dictation_input: string;   // Derived from the dictation trigger
    stop_current_task: string; // Always Escape
    open_settings: string;     // Always Cmd+, (Ctrl+, on non-macOS)
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
