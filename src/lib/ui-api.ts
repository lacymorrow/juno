/**
 * Standardized UI API for all floating components in Juno
 * Supports bars, panels, chat interfaces, and any floating UI element
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";

// === Core UI Types ===

export type UIElementType = "bar" | "panel" | "chat" | "overlay" | "modal";

export type UIState =
    | "default"
    | "expanding"
    | "expanded"
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
    | "agent-responding"
    | "dictation-ready";

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
    private currentState: UIStateData | null = null;

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
            this.currentState = state;
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
            await invoke("ui_handle_window", {
                elementId: this.elementId,
                windowEvent: {
                    type: "resize",
                    elementId: this.elementId,
                    data: { width, height }
                }
            });
            return true;
        } catch (error) {
            console.error(`Failed to resize window for ${this.elementId}:`, error);
            return false;
        }
    }

    async moveWindow(x: number, y: number): Promise<boolean> {
        try {
            await invoke("ui_handle_window", {
                elementId: this.elementId,
                windowEvent: {
                    type: "move",
                    elementId: this.elementId,
                    data: { x, y }
                }
            });
            return true;
        } catch (error) {
            console.error(`Failed to move window for ${this.elementId}:`, error);
            return false;
        }
    }

    async showWindow(): Promise<boolean> {
        try {
            await invoke("ui_handle_window", {
                elementId: this.elementId,
                windowEvent: { type: "show", elementId: this.elementId }
            });
            return true;
        } catch (error) {
            console.error(`Failed to show window for ${this.elementId}:`, error);
            return false;
        }
    }

    async hideWindow(): Promise<boolean> {
        try {
            await invoke("ui_handle_window", {
                elementId: this.elementId,
                windowEvent: { type: "hide", elementId: this.elementId }
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
                clickThrough: enabled
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
            this.currentState = event.payload.data;
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
        case "default":
            return "✨";
        case "dictating":
            return "📝";
        case "listening":
            return voiceMode === "dictation" ? "📝" : "🎤";
        case "transcribing":
            return "🔄";
        case "loading":
        case "submitting":
            return "⏳";
        case "agent-responding":
            return "🧠";
        case "speaking":
            return "🔊";
        case "success":
            return "✅";
        case "error":
            return "❌";
        case "always-listening":
            return "👂";
        default:
            return "💫";
    }
}

export function getStateColor(state: UIState, voiceMode: VoiceMode = "idle"): string {
    switch (state) {
        case "dictating":
        case "transcribing":
            return voiceMode === "dictation" ? "orange" : "blue";
        case "listening":
            return voiceMode === "dictation" ? "orange" : "blue";
        case "loading":
        case "submitting":
        case "agent-responding":
            return "blue";
        case "speaking":
            return "purple";
        case "success":
            return "green";
        case "error":
            return "red";
        case "always-listening":
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
        case "default":
            return "Ready";
        case "dictating":
            return voiceMode === "dictation" ? "Dictating..." : "Listening for command...";
        case "listening":
            return "Listening...";
        case "transcribing":
            return "Processing...";
        case "loading":
            return "AI thinking...";
        case "submitting":
            return "Submitting...";
        case "agent-responding":
            return "AI responding...";
        case "speaking":
            return "Speaking...";
        case "success":
            if (agentStatus === "failed") return "Task failed";
            if (agentStatus === "cancelled") return "Task cancelled";
            if (agentStatus === "offline") return "Connection unavailable";
            return "Task completed";
        case "error":
            return error || "Error occurred";
        case "always-listening":
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
