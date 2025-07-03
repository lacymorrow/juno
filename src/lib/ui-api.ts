/**
 * Standardized UI API for all floating components in Juno
 * Supports bars, panels, chat interfaces, and any floating UI element
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { useState, useEffect } from "react";
import { UI } from "./constants.generated";

// === Core UI Types ===

export type UIElementType = "bar" | "panel" | "chat" | "overlay" | "modal";

export type UIState =
    | typeof UI.BAR_STATES_DEFAULT
    | typeof UI.BAR_STATES_EXPANDING
    | typeof UI.BAR_STATES_INPUT
    | typeof UI.BAR_STATES_SHRINKING
    | typeof UI.BAR_STATES_SUBMITTING
    | typeof UI.BAR_STATES_LOADING
    | typeof UI.BAR_STATES_FINISHING
    | typeof UI.BAR_STATES_SUCCESS
    | typeof UI.BAR_STATES_LISTENING
    | typeof UI.BAR_STATES_ERROR
    | typeof UI.BAR_STATES_TRANSCRIBING
    | typeof UI.BAR_STATES_SPEAKING
    | typeof UI.BAR_STATES_DICTATING
    | typeof UI.BAR_STATES_ALWAYS_LISTENING
    | typeof UI.BAR_STATES_AGENT_RESPONDING
    | typeof UI.BAR_STATES_DICTATION_READY;

export type VoiceMode = "idle" | "agent" | "dictation";

export type AgentStatus = "idle" | "working" | "responding" | "finished" | "failed" | "cancelled" | "offline";

// === Configuration Types ===

export interface UIElementConfig {
    id: string;
    type: UIElementType;
    showVoiceIndicator: boolean;
    enableAnimations: boolean;
    autoHide: boolean;
    autoHideDelay: number;
    opacity: number;
    position?: { x: number; y: number };
    dimensions?: { width: number; height: number };
    clickThrough?: boolean;
    alwaysOnTop?: boolean;
}

// === State Types ===

export interface UIStateData {
    elementId: string;
    elementType: UIElementType;
    uiState: UIState;
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
    agentState: AgentStatus;
    timestamp: number;
}

// === Event Types ===

export interface UIEventPayload<T = any> {
    elementId: string;
    elementType: UIElementType;
    data: T;
    timestamp: number;
}

export interface UIInteractionEvent {
    type: "click" | "focus" | "blur" | "hover" | "input" | "submit";
    elementId: string;
    data?: any;
}

export interface UIWindowEvent {
    type: "resize" | "move" | "show" | "hide" | "focus" | "blur";
    elementId: string;
    data?: any;
}

// === Core UI Manager Class ===

export class UIElementManager {
    private elementId: string;
    private elementType: UIElementType;
    private listeners: Map<string, UnlistenFn> = new Map();


    constructor(elementId: string, elementType: UIElementType) {
        this.elementId = elementId;
        this.elementType = elementType;
    }

    // === State Management ===

    async getState(): Promise<UIStateData | null> {
        try {
            const state = await invoke<UIStateData>("ui_get_state", {
                elementId: this.elementId
            });

            return state;
        } catch (error) {
            console.error(`Failed to get state for ${this.elementId}:`, error);
            return null;
        }
    }

    async setState(newState: Partial<UIStateData>): Promise<boolean> {
        try {
            await invoke("ui_set_state", {
                elementId: this.elementId,
                elementType: this.elementType,
                stateUpdate: newState
            });
            return true;
        } catch (error) {
            console.error(`Failed to set state for ${this.elementId}:`, error);
            return false;
        }
    }

    // === Configuration Management ===

    async getConfig(): Promise<UIElementConfig | null> {
        try {
            return await invoke<UIElementConfig>("ui_get_config", {
                elementId: this.elementId
            });
        } catch (error) {
            console.error(`Failed to get config for ${this.elementId}:`, error);
            return null;
        }
    }

    async setConfig(config: Partial<UIElementConfig>): Promise<boolean> {
        try {
            await invoke("ui_set_config", {
                elementId: this.elementId,
                config: { ...config, id: this.elementId, type: this.elementType }
            });
            return true;
        } catch (error) {
            console.error(`Failed to set config for ${this.elementId}:`, error);
            return false;
        }
    }

    // === Interaction Commands ===

    async click(data?: any): Promise<boolean> {
        try {
            await invoke("ui_handle_interaction", {
                elementId: this.elementId,
                interaction: { type: "click", elementId: this.elementId, data }
            });
            return true;
        } catch (error) {
            console.error(`Failed to handle click for ${this.elementId}:`, error);
            return false;
        }
    }

    async focus(data?: any): Promise<boolean> {
        try {
            await invoke("ui_handle_interaction", {
                elementId: this.elementId,
                interaction: { type: "focus", elementId: this.elementId, data }
            });
            return true;
        } catch (error) {
            console.error(`Failed to handle focus for ${this.elementId}:`, error);
            return false;
        }
    }

    async blur(data?: any): Promise<boolean> {
        try {
            await invoke("ui_handle_interaction", {
                elementId: this.elementId,
                interaction: { type: "blur", elementId: this.elementId, data }
            });
            return true;
        } catch (error) {
            console.error(`Failed to handle blur for ${this.elementId}:`, error);
            return false;
        }
    }

    async input(value: string): Promise<boolean> {
        try {
            await invoke("ui_handle_interaction", {
                elementId: this.elementId,
                interaction: { type: "input", elementId: this.elementId, data: { value } }
            });
            return true;
        } catch (error) {
            console.error(`Failed to handle input for ${this.elementId}:`, error);
            return false;
        }
    }

    async submit(query: string): Promise<boolean> {
        try {
            await invoke("ui_handle_interaction", {
                elementId: this.elementId,
                interaction: { type: "submit", elementId: this.elementId, data: { query } }
            });
            return true;
        } catch (error) {
            console.error(`Failed to handle submit for ${this.elementId}:`, error);
            return false;
        }
    }

    // === Window Management ===

    async resizeWindow(width: number, height: number): Promise<boolean> {
        try {
            await invoke("ui_resize_window", {
                elementId: this.elementId,
                width,
                height
            });
            return true;
        } catch (error) {
            console.error(`Failed to resize window for ${this.elementId}:`, error);
            return false;
        }
    }

    async moveWindow(x: number, y: number): Promise<boolean> {
        try {
            await invoke("ui_move_window", {
                elementId: this.elementId,
                x,
                y
            });
            return true;
        } catch (error) {
            console.error(`Failed to move window for ${this.elementId}:`, error);
            return false;
        }
    }

    async showWindow(): Promise<boolean> {
        try {
            await invoke("ui_show_window", {
                elementId: this.elementId
            });
            return true;
        } catch (error) {
            console.error(`Failed to show window for ${this.elementId}:`, error);
            return false;
        }
    }

    async hideWindow(): Promise<boolean> {
        try {
            await invoke("ui_hide_window", {
                elementId: this.elementId
            });
            return true;
        } catch (error) {
            console.error(`Failed to hide window for ${this.elementId}:`, error);
            return false;
        }
    }

    async setClickThrough(enabled: boolean): Promise<boolean> {
        try {
            await invoke("ui_set_click_through", {
                elementId: this.elementId,
                enabled
            });
            return true;
        } catch (error) {
            console.error(`Failed to set click-through for ${this.elementId}:`, error);
            return false;
        }
    }

    async setWindowLevel(level: number): Promise<boolean> {
        try {
            await invoke("ui_set_window_level", {
                elementId: this.elementId,
                level
            });
            return true;
        } catch (error) {
            console.error(`Failed to set window level for ${this.elementId}:`, error);
            return false;
        }
    }

    // === Event Management ===

    async onStateUpdate(callback: (state: UIStateData) => void): Promise<UnlistenFn> {
        const eventName = `ui-state-update-${this.elementId}`;
        const unlisten = await listen<UIEventPayload<UIStateData>>(eventName, (event) => {
            callback(event.payload.data);
        });
        this.listeners.set("state-update", unlisten);
        return unlisten;
    }

    async onConfigUpdate(callback: (config: UIElementConfig) => void): Promise<UnlistenFn> {
        const eventName = `ui-config-update-${this.elementId}`;
        const unlisten = await listen<UIEventPayload<UIElementConfig>>(eventName, (event) => {
            callback(event.payload.data);
        });
        this.listeners.set("config-update", unlisten);
        return unlisten;
    }

    async onAgentEvent(callback: (agentData: any) => void): Promise<UnlistenFn> {
        const eventName = `ui-agent-event-${this.elementId}`;
        const unlisten = await listen<UIEventPayload<any>>(eventName, (event) => {
            callback(event.payload.data);
        });
        this.listeners.set("agent-event", unlisten);
        return unlisten;
    }

    async onVoiceEvent(callback: (voiceData: any) => void): Promise<UnlistenFn> {
        const eventName = `ui-voice-event-${this.elementId}`;
        const unlisten = await listen<UIEventPayload<any>>(eventName, (event) => {
            callback(event.payload.data);
        });
        this.listeners.set("voice-event", unlisten);
        return unlisten;
    }

    // === Cleanup ===

    async cleanup(): Promise<void> {
        for (const [key, unlisten] of this.listeners) {
            try {
                await unlisten();
            } catch (error) {
                console.warn(`Failed to unlisten ${key} for ${this.elementId}:`, error);
            }
        }
        this.listeners.clear();
    }
}

// === Factory Functions ===

export function createUIElement(elementId: string, elementType: UIElementType): UIElementManager {
    return new UIElementManager(elementId, elementType);
}

// === Utility Functions ===

export function getStateIcon(state: UIState, voiceMode: VoiceMode = "idle"): string {
    switch (state) {
        case UI.BAR_STATES_DEFAULT:
            return "✨";
        case UI.BAR_STATES_DICTATING:
            return "📝";
        case UI.BAR_STATES_LISTENING:
            return voiceMode === "dictation" ? "📝" : "🎤";
        case UI.BAR_STATES_TRANSCRIBING:
            return "🔄";
        case UI.BAR_STATES_LOADING:
        case UI.BAR_STATES_SUBMITTING:
            return "⏳";
        case UI.BAR_STATES_AGENT_RESPONDING:
            return "🧠";
        case UI.BAR_STATES_SPEAKING:
            return "🔊";
        case UI.BAR_STATES_SUCCESS:
            return "✅";
        case UI.BAR_STATES_ERROR:
            return "❌";
        case UI.BAR_STATES_ALWAYS_LISTENING:
            return "👂";
        default:
            return "💫";
    }
}

export function getStateColor(state: UIState, voiceMode: VoiceMode = "idle"): string {
    switch (state) {
        case UI.BAR_STATES_DICTATING:
        case UI.BAR_STATES_TRANSCRIBING:
            return voiceMode === "dictation" ? "orange" : "blue";
        case UI.BAR_STATES_LISTENING:
            return voiceMode === "dictation" ? "orange" : "blue";
        case UI.BAR_STATES_LOADING:
        case UI.BAR_STATES_SUBMITTING:
        case UI.BAR_STATES_AGENT_RESPONDING:
            return "blue";
        case UI.BAR_STATES_SPEAKING:
            return "purple";
        case UI.BAR_STATES_SUCCESS:
            return "green";
        case UI.BAR_STATES_ERROR:
            return "red";
        case UI.BAR_STATES_ALWAYS_LISTENING:
            return "cyan";
        default:
            return "gray";
    }
}

export function getStateDescription(
    state: UIState,
    voiceMode: VoiceMode = "idle",
    agentStatus: AgentStatus = "idle",
    error?: string
): string {
    switch (state) {
        case UI.BAR_STATES_DEFAULT:
            return "Ready";
        case UI.BAR_STATES_DICTATING:
            return voiceMode === "dictation" ? "Dictating..." : "Listening for command...";
        case UI.BAR_STATES_LISTENING:
            return "Listening...";
        case UI.BAR_STATES_TRANSCRIBING:
            return "Processing...";
        case UI.BAR_STATES_LOADING:
            return "AI thinking...";
        case UI.BAR_STATES_SUBMITTING:
            return "Submitting...";
        case UI.BAR_STATES_AGENT_RESPONDING:
            return "AI responding...";
        case UI.BAR_STATES_SPEAKING:
            return "Speaking...";
        case UI.BAR_STATES_SUCCESS:
            if (agentStatus === "failed") return "Task failed";
            if (agentStatus === "cancelled") return "Task cancelled";
            if (agentStatus === "offline") return "Connection unavailable";
            return "Task completed";
        case UI.BAR_STATES_ERROR:
            return error || "Error occurred";
        case UI.BAR_STATES_ALWAYS_LISTENING:
            return "Always listening...";
        default:
            return "Ready";
    }
}

// === Hook for React Components ===

export function useUIElement(elementId: string, elementType: UIElementType) {
    const [manager] = useState(() => createUIElement(elementId, elementType));
    const [state, setState] = useState<UIStateData | null>(null);
    const [config, setConfig] = useState<UIElementConfig | null>(null);

    useEffect(() => {
        let mounted = true;

        // Load initial state and config
        const initialize = async () => {
            const [initialState, initialConfig] = await Promise.all([
                manager.getState(),
                manager.getConfig()
            ]);

            if (mounted) {
                setState(initialState);
                setConfig(initialConfig);
            }
        };

        // Set up event listeners
        const setupListeners = async () => {
            await manager.onStateUpdate((newState) => {
                if (mounted) setState(newState);
            });

            await manager.onConfigUpdate((newConfig) => {
                if (mounted) setConfig(newConfig);
            });
        };

        initialize();
        setupListeners();

        return () => {
            mounted = false;
            manager.cleanup();
        };
    }, [manager]);

    return {
        manager,
        state,
        config,
        // Helper methods
        click: manager.click.bind(manager),
        focus: manager.focus.bind(manager),
        blur: manager.blur.bind(manager),
        input: manager.input.bind(manager),
        submit: manager.submit.bind(manager),
        resizeWindow: manager.resizeWindow.bind(manager),
        moveWindow: manager.moveWindow.bind(manager),
        showWindow: manager.showWindow.bind(manager),
        hideWindow: manager.hideWindow.bind(manager),
        setClickThrough: manager.setClickThrough.bind(manager),
        setWindowLevel: manager.setWindowLevel.bind(manager),
        updateConfig: (newConfig: Partial<UIElementConfig>) => manager.setConfig(newConfig),
        updateState: (newState: Partial<UIStateData>) => manager.setState(newState)
    };
}

export default UIElementManager;
