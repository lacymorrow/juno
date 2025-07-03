/**
 * FloatingBar.tsx - Standardized UI API Example
 *
 * This component demonstrates the proper patterns for UI component backend integration:
 * 1. Event-driven state updates via "bar-state-update" events
 * 2. User interactions via ui_handle_interaction command
 * 3. Type-safe inline type definitions aligned with backend
 * 4. Comprehensive error handling and logging
 * 5. Proper window resizing and state management
 *
 * This serves as the reference implementation for all floating UI components.
 */

import { useEffect, useState, useCallback, FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import {
  Mic,
  Sparkles,
  Brain,
  Loader2,
  Volume2,
  Check,
  AlertCircle,
  Type,
  Send,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import tauriConfig from "../../src-tauri/tauri.conf.json";

// === STANDARDIZED UI API TYPES ===

/**
 * UI State enumeration - MUST match backend BarState.as_str() exactly
 * These values are emitted by the backend UIManager in bar-state-update events
 */
type UIState =
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
  | "dictation_ready" // Note: backend uses underscore, not kebab-case
  | "always_listening" // Note: backend uses underscore, not kebab-case
  | "agent_responding"; // Note: backend uses underscore, not kebab-case

/**
 * Backend State Data Structure - Matches exactly what backend emits
 * This structure is defined in ui_commands.rs emit_bar_state_update()
 */
interface BarStateData {
  // Core state
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;

  // Voice and transcription
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;

  // Status flags
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;

  // Agent state
  agentState: string | null;
}

/**
 * Standardized UI Interaction Event Structure
 * This matches UIInteractionEvent in ui_commands.rs
 */
interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

// === COMPONENT CONSTANTS ===

const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 60,
  DEFAULT_HEIGHT: 20,
  EXPANDED_WIDTH: 280,
  EXPANDED_HEIGHT: 50,
};

/**
 * Component name for backend interactions - MUST match backend element handling
 */
const COMPONENT_ID = "floating-bar";

// === MAIN COMPONENT ===

export function FloatingBar() {
  // === STATE MANAGEMENT ===

  /**
   * Backend-driven state - Updated via bar-state-update events
   * This is the single source of truth for all UI state
   */
  const [barState, setBarState] = useState<BarStateData>({
    barState: "default",
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: "idle",
    agentState: null,
  });

  // === WINDOW CONFIGURATION ===

  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === "floating-bar"
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
  const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
  const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

  // === STANDARDIZED EVENT LISTENER ===

  /**
   * Primary backend integration: Listen to bar-state-update events
   * This is the core pattern for all UI components - event-driven state updates
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>("bar-state-update", (event) => {
          console.log("📨 FloatingBar: Received state update:", event.payload);

          // Validate the received data structure
          const payload = event.payload;
          if (payload && typeof payload === "object" && "barState" in payload) {
            setBarState(payload);
          } else {
            console.error(
              "❌ FloatingBar: Invalid state data received:",
              payload
            );
          }
        });

        console.log("✅ FloatingBar: Event listener established");
      } catch (error) {
        console.error("❌ FloatingBar: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        console.log("🔄 FloatingBar: Event listener cleaned up");
      }
    };
  }, []);

  // === WINDOW RESIZING LOGIC ===

  /**
   * Responsive window resizing based on UI state
   * Compact states use small dimensions, expanded states use larger dimensions
   */
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const currentUiState = barState.barState;

        // Define compact states that use small window size
        const isCompact = ["default", "dictation_ready"].includes(
          currentUiState
        );
        const currentWidth = isCompact ? defaultWidth : EXPANDED_WIDTH;
        const currentHeight = isCompact ? defaultHeight : EXPANDED_HEIGHT;

        console.log(
          `🔧 FloatingBar: Resizing window to ${currentWidth}x${currentHeight} for state: ${currentUiState}`
        );

        await appWindow.setSize(new LogicalSize(currentWidth, currentHeight));
      } catch (error) {
        console.error("❌ FloatingBar: Failed to resize window:", error);
      }
    };

    resizeWindow();
  }, [barState.barState]); // ✅ Only depend on the actual state that changes

  // === STANDARDIZED INTERACTION HANDLERS ===

  /**
   * Creates a standardized UI interaction event
   * This helper ensures all interactions follow the same pattern
   */
  const createInteraction = (
    interactionType: string,
    data?: Record<string, any>
  ): UIInteractionEvent => ({
    element_id: COMPONENT_ID,
    interaction_type: interactionType,
    data: data || null,
    timestamp: Date.now(),
  });

  /**
   * Sends interaction to backend via ui_handle_interaction command
   * This is the standardized way to trigger backend actions
   */
  const sendInteraction = async (interaction: UIInteractionEvent) => {
    try {
      console.log("🔧 FloatingBar: Sending interaction:", interaction);

      await invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction,
      });

      console.log("✅ FloatingBar: Interaction sent successfully");
    } catch (error) {
      console.error("❌ FloatingBar: Interaction failed:", error);

      // TODO: Could emit error event or show user notification
      // This demonstrates proper error handling without breaking the UI
    }
  };

  /**
   * Handle bar click - Demonstrates simple interaction
   */
  const handleClick = useCallback(async () => {
    const interaction = createInteraction("click");
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle input changes - Demonstrates data-carrying interaction
   */
  const handleInputChange = useCallback(async (value: string) => {
    const interaction = createInteraction("input_change", { value });
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle form submission - Demonstrates validation + interaction
   */
  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const trimmedValue = barState.inputValue.trim();

      if (trimmedValue) {
        const interaction = createInteraction("submit", {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      } else {
        console.log("⚠️ FloatingBar: Empty submission ignored");
      }
    },
    [barState.inputValue]
  );

  /**
   * Handle focus events - Demonstrates state-aware interactions
   */
  const handleFocus = useCallback(async () => {
    const interaction = createInteraction("focus", { is_focused: true });
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle blur events - Demonstrates state-aware interactions
   */
  const handleBlur = useCallback(async () => {
    const interaction = createInteraction("blur", { is_focused: false });
    await sendInteraction(interaction);
  }, []);

  // === UI STATE CALCULATIONS ===

  const currentUiState = barState.barState;
  const isCompact = ["default", "dictation_ready"].includes(currentUiState);
  const currentWidth = isCompact ? defaultWidth : EXPANDED_WIDTH;
  const currentHeight = isCompact ? defaultHeight : EXPANDED_HEIGHT;

  // === DYNAMIC STYLING SYSTEM ===

  /**
   * Generates container styles based on current state
   * Demonstrates responsive styling based on backend state
   */
  const getContainerStyles = () => {
    const sizeStyles = isCompact
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const clickable = ["default", "dictation_ready"].includes(currentUiState)
      ? "cursor-pointer"
      : "";

    return cn(
      "relative flex items-center justify-center",
      "text-white rounded-full shadow-lg border border-white/20",
      "transition-all duration-300 ease-in-out",
      "bg-black/90 backdrop-blur-md",
      sizeStyles,
      clickable
    );
  };

  // === VISUAL HELPERS ===

  /**
   * Returns appropriate icon for current state
   * Demonstrates state-driven visual feedback
   */
  const getMainIcon = () => {
    switch (currentUiState) {
      case "listening":
        return <Mic size={14} className="text-blue-400" />;
      case "transcribing":
        return <Mic size={14} className="animate-pulse text-blue-400" />;
      case "speaking":
        return <Volume2 size={14} className="text-green-400" />;
      case "loading":
      case "submitting":
        return <Loader2 size={14} className="animate-spin text-yellow-400" />;
      case "input":
      case "expanding":
        return <Sparkles size={14} className="text-white" />;
      case "dictation_ready":
        return <Type size={14} className="text-orange-400" />;
      case "always_listening":
        return <Mic size={14} className="text-blue-400 animate-pulse" />;
      case "error":
        return <AlertCircle size={14} className="text-red-400" />;
      case "success":
        return <Check size={14} className="text-green-400" />;
      default:
        return <Brain size={14} className="text-white" />;
    }
  };

  /**
   * Audio level visualization component
   * Shows real-time audio feedback from backend
   */
  const AudioLevelIndicator = ({ audioLevel }: { audioLevel: number }) => {
    if (
      !["listening", "transcribing", "always_listening"].includes(
        currentUiState
      )
    ) {
      return null;
    }

    const normalizedLevel = Math.min(Math.max(audioLevel * 100, 0), 100);
    const barCount = Math.ceil(normalizedLevel / 20);

    return (
      <div className="flex items-center gap-0.5">
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            className={cn(
              "w-0.5 h-2 rounded-full transition-all duration-100",
              i < barCount ? "bg-blue-400" : "bg-white/20"
            )}
          />
        ))}
      </div>
    );
  };

  // === RENDER LOGIC ===

  return (
    <div className="w-screen h-screen relative overflow-hidden cursor-move">
      <div
        className={getContainerStyles()}
        style={{
          width: `${currentWidth}px`,
          height: `${currentHeight}px`,
        }}
        onClick={
          ["default", "dictation_ready"].includes(currentUiState)
            ? handleClick
            : undefined
        }
      >
        {/* Compact States - Default and Dictation Ready */}
        {isCompact && (
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon()}
            {barState.voiceMode !== "idle" && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          </div>
        )}

        {/* Active States with Audio Feedback */}
        {[
          "listening",
          "transcribing",
          "speaking",
          "dictating",
          "agent_responding",
        ].includes(currentUiState) && (
          <div
            className="flex items-center justify-between w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon()}
              <span
                className="text-sm font-medium truncate"
                data-tauri-drag-region
              >
                {currentUiState === "listening" && "Listening..."}
                {currentUiState === "transcribing" && "Converting speech..."}
                {currentUiState === "speaking" && "Playing response..."}
                {currentUiState === "dictating" && "Dictating text..."}
                {currentUiState === "agent_responding" && "Agent working..."}
              </span>
            </div>
            <AudioLevelIndicator audioLevel={barState.audioLevel} />
          </div>
        )}

        {/* Input State - Interactive Form */}
        {(currentUiState === "expanding" || currentUiState === "input") && (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3",
              "transition-opacity duration-300 ease-in-out",
              currentUiState === "input" ? "opacity-100" : "opacity-0"
            )}
            data-tauri-drag-region
          >
            <div
              className="flex items-center gap-2 flex-1"
              data-tauri-drag-region
            >
              {getMainIcon()}
              <input
                type="text"
                value={barState.inputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onFocus={handleFocus}
                onBlur={handleBlur}
                placeholder="Ask me anything..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                disabled={currentUiState !== "input"}
              />
            </div>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={currentUiState !== "input"}
            >
              <Send size={14} />
            </button>
          </form>
        )}

        {/* Status States - Loading, Error, Success */}
        {[
          "submitting",
          "loading",
          "error",
          "success",
          "shrinking",
          "finishing",
        ].includes(currentUiState) && (
          <div
            className="flex items-center justify-center w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon()}
              <span className="text-sm font-medium" data-tauri-drag-region>
                {currentUiState === "submitting" && "Sending..."}
                {currentUiState === "loading" && "Processing..."}
                {currentUiState === "finishing" && "Finishing..."}
                {currentUiState === "error" &&
                  (barState.currentError || "Error occurred")}
                {currentUiState === "success" && "Complete!"}
                {currentUiState === "shrinking" && ""}
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * STANDARDIZED UI API PATTERNS DEMONSTRATED:
 *
 * ✅ Event-Driven State Updates:
 *    - listen("bar-state-update", handler)
 *    - Single source of truth from backend
 *    - Type-safe payload validation
 *
 * ✅ Command-Based User Interactions:
 *    - invoke("ui_handle_interaction", { elementId, interaction })
 *    - Standardized interaction event structure
 *    - Comprehensive error handling
 *
 * ✅ Type Safety & Backend Alignment:
 *    - Types match backend exactly (underscore notation)
 *    - Interface structure mirrors backend emission
 *    - Inline type definitions (no external dependencies)
 *
 * ✅ Robust Error Handling:
 *    - Try/catch on all backend calls
 *    - Payload validation on events
 *    - Graceful degradation on failures
 *
 * ✅ Performance & UX:
 *    - Responsive window resizing
 *    - Smooth state transitions
 *    - Audio feedback integration
 *
 * ✅ Maintainable Architecture:
 *    - Clear separation of concerns
 *    - Reusable interaction patterns
 *    - Comprehensive documentation
 *
 * This component serves as the reference implementation for all
 * floating UI components in the Juno application.
 */
