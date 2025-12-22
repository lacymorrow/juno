/**
 * FloatingBar.tsx - Standardized UI API Example
 *
 * This component demonstrates the proper patterns for UI component backend integration:
 * 1. Event-driven state updates via BAR_STATE_UPDATE events
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
// No direct window calls needed; resizing is handled by hook
import { useWindowSize } from "@/hooks/useWindowSize";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import type { BarAppearance } from "@/components/bar/barAppearance";
import { getBarLayoutWindowLabel } from "@/components/bar/barAppearance";

// === STANDARDIZED UI API TYPES ===

/**
 * UI State enumeration - Uses generated constants from backend
 * These values are emitted by the backend UIManager in BAR_STATE_UPDATE events
 */
type UIState =
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
  | typeof UI.BAR_STATES_DICTATION_READY
  | typeof UI.BAR_STATES_ALWAYS_LISTENING
  | typeof UI.BAR_STATES_AGENT_RESPONDING;

/**
 * BarStateData interface - The state object emitted by the backend
 * This is shared across all UI components and provides a comprehensive state model
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
  SHADOW_PADDING: 48, // 24px per side
};

/**
 * Component name for backend interactions - MUST match backend element handling
 */
const COMPONENT_ID = "floating-bar";

// === MAIN COMPONENT ===

export function FloatingBar({ barAppearance }: { barAppearance?: BarAppearance }) {
  // === STATE MANAGEMENT ===

  /**
   * Backend-driven state - Updated via BAR_STATE_UPDATE events
   * This is the single source of truth for all UI state
   */
  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: UI.VOICE_MODES_IDLE,
    agentState: null,
  });

  // === WINDOW CONFIGURATION ===
  const windowLabel = getCurrentWindow().label;
  const layoutWindowLabel = barAppearance
    ? getBarLayoutWindowLabel(barAppearance)
    : windowLabel;
  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === layoutWindowLabel
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
  const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
  const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

  // === STANDARDIZED EVENT LISTENER ===

  /**
   * Primary backend integration: Listen to both direct and forwarded events
   * 1. Listen directly to BAR_STATE_UPDATE events from the backend
   * 2. Listen to component-specific events from the multi-bar-registration system
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    // Handler function for processing state updates
    const handleStateUpdate = (payload: any) => {
      console.log("📨 FloatingBar: Received state update:", payload);

      // Validate the received data structure
      if (payload && typeof payload === "object" && "barState" in payload) {
        setBarState(payload);
      } else {
        console.error("❌ FloatingBar: Invalid state data received:", payload);
      }
    };

    const setupListener = async () => {
      try {
        // Set up the standard backend event listener
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => handleStateUpdate(event.payload)
        );

        // Also listen for component-specific events from event forwarding
        document.addEventListener(`${COMPONENT_ID}-state-update`, ((
          e: CustomEvent
        ) => handleStateUpdate(e.detail)) as EventListener);

        console.log("✅ FloatingBar: Event listeners established");
      } catch (error) {
        console.error(
          "❌ FloatingBar: Failed to setup event listeners:",
          error
        );
      }
    };

    setupListener();

    return () => {
      // Clean up both listeners
      if (unlisten) {
        unlisten();
      }
      document.removeEventListener(`${COMPONENT_ID}-state-update`, ((
        e: CustomEvent
      ) => handleStateUpdate(e.detail)) as EventListener);
      console.log("🔄 FloatingBar: Event listeners cleaned up");
    };
  }, []);

  // === WINDOW RESIZING LOGIC ===

  /**
   * Responsive window resizing based on UI state
   * Compact states use small dimensions, expanded states use larger dimensions
   */
  // lastSizeRef not required; managed inside useWindowSize
  const { resizeWindowIfChanged } = useWindowSize(windowLabel);
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        // Window instance not needed; resize handled by custom hook
        const currentUiState = barState.barState;

        const isCompact = [
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_DICTATION_READY,
        ].includes(currentUiState as any);
        const targetWidth = (isCompact ? defaultWidth : EXPANDED_WIDTH) + FLOATING_BAR_DIMENSIONS.SHADOW_PADDING;
        const targetHeight = (isCompact ? defaultHeight : EXPANDED_HEIGHT) + FLOATING_BAR_DIMENSIONS.SHADOW_PADDING;

        await resizeWindowIfChanged({ width: targetWidth, height: targetHeight });
      } catch (error) {
        console.error("❌ FloatingBar: Failed to resize window:", error);
      }
    };

    resizeWindow();
  }, [barState.barState]);

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
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, []);

  /**
   * Local input state management - No backend interaction needed
   * The component manages its own input state and only sends the final value on submit
   */
  const [localInputValue, setLocalInputValue] = useState("");

  /**
   * Handle input changes locally - No backend interaction
   * This is more efficient than broadcasting every keystroke
   */
  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
    // No backend interaction needed for input changes
  }, []);

  /**
   * Handle form submission - Demonstrates validation + interaction
   */
  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const trimmedValue = localInputValue.trim();

      if (trimmedValue) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_SUBMIT, {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      } else {
        console.log("⚠️ FloatingBar: Empty submission ignored");
      }
    },
    [localInputValue]
  );

  /**
   * Handle focus events - Demonstrates state-aware interactions
   */
  const handleFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS, {
      is_focused: true,
    });
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle blur events - Demonstrates state-aware interactions
   */
  const handleBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR, {
      is_focused: false,
    });
    await sendInteraction(interaction);
  }, []);

  // === SYNC LOCAL INPUT WITH BACKEND STATE ===

  /**
   * Sync local input state with backend state updates
   * This ensures the input reflects the backend's current state
   */
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  // === UI STATE CALCULATIONS ===

  const currentUiState = barState.barState;
  const isCompact = [
    UI.BAR_STATES_DEFAULT,
    UI.BAR_STATES_DICTATION_READY,
  ].includes(currentUiState as any);
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

    const clickable = [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_DICTATION_READY,
    ].includes(currentUiState as any)
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
      case UI.BAR_STATES_LISTENING:
        return <Mic size={14} className="text-blue-400" />;
      case UI.BAR_STATES_TRANSCRIBING:
        return <Mic size={14} className="animate-pulse text-blue-400" />;
      case UI.BAR_STATES_SPEAKING:
        return <Volume2 size={14} className="text-green-400" />;
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return <Loader2 size={14} className="animate-spin text-yellow-400" />;
      case UI.BAR_STATES_INPUT:
      case UI.BAR_STATES_EXPANDING:
        return <Sparkles size={14} className="text-white" />;
      case UI.BAR_STATES_DICTATION_READY:
        return <Type size={14} className="text-orange-400" />;
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return <Mic size={14} className="text-blue-400 animate-pulse" />;
      case UI.BAR_STATES_ERROR:
        return <AlertCircle size={14} className="text-red-400" />;
      case UI.BAR_STATES_SUCCESS:
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
      ![
        UI.BAR_STATES_LISTENING,
        UI.BAR_STATES_TRANSCRIBING,
        UI.BAR_STATES_ALWAYS_LISTENING,
      ].includes(currentUiState as any)
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
    <div className="w-screen h-screen relative overflow-hidden cursor-move flex items-center justify-center p-6">
      <div
        className={getContainerStyles()}
        style={{
          width: `${currentWidth}px`,
          height: `${currentHeight}px`,
        }}
        onClick={
          [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
            currentUiState as any
          )
            ? handleClick
            : undefined
        }
      >
        {/* Compact States - Default and Dictation Ready */}
        {isCompact && (
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon()}
            {barState.voiceMode !== UI.VOICE_MODES_IDLE && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          </div>
        )}

        {/* Active States with Audio Feedback */}
        {[
          UI.BAR_STATES_LISTENING,
          UI.BAR_STATES_TRANSCRIBING,
          UI.BAR_STATES_SPEAKING,
          UI.BAR_STATES_DICTATING,
          UI.BAR_STATES_AGENT_RESPONDING,
        ].includes(currentUiState as any) && (
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
                {currentUiState === UI.BAR_STATES_LISTENING && "Listening..."}
                {currentUiState === UI.BAR_STATES_TRANSCRIBING &&
                  "Converting speech..."}
                {currentUiState === UI.BAR_STATES_SPEAKING &&
                  "Playing response..."}
                {currentUiState === UI.BAR_STATES_DICTATING &&
                  "Dictating text..."}
                {currentUiState === UI.BAR_STATES_AGENT_RESPONDING &&
                  "Agent working..."}
              </span>
            </div>
            <AudioLevelIndicator audioLevel={barState.audioLevel} />
          </div>
        )}

        {/* Input State - Interactive Form */}
        {(currentUiState === UI.BAR_STATES_EXPANDING ||
          currentUiState === UI.BAR_STATES_INPUT) && (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3",
              "transition-opacity duration-300 ease-in-out",
              currentUiState === UI.BAR_STATES_INPUT
                ? "opacity-100"
                : "opacity-0"
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
                value={localInputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onFocus={handleFocus}
                onBlur={handleBlur}
                placeholder="Ask me anything..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                disabled={currentUiState !== UI.BAR_STATES_INPUT}
              />
            </div>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={currentUiState !== UI.BAR_STATES_INPUT}
            >
              <Send size={14} />
            </button>
          </form>
        )}

        {/* Status States - Loading, Error, Success */}
        {[
          UI.BAR_STATES_SUBMITTING,
          UI.BAR_STATES_LOADING,
          UI.BAR_STATES_ERROR,
          UI.BAR_STATES_SUCCESS,
          UI.BAR_STATES_SHRINKING,
          UI.BAR_STATES_FINISHING,
        ].includes(currentUiState as any) && (
          <div
            className="flex items-center justify-center w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon()}
              <span className="text-sm font-medium" data-tauri-drag-region>
                {currentUiState === UI.BAR_STATES_SUBMITTING && "Sending..."}
                {currentUiState === UI.BAR_STATES_LOADING && "Processing..."}
                {currentUiState === UI.BAR_STATES_FINISHING && "Finishing..."}
                {currentUiState === UI.BAR_STATES_ERROR &&
                  (barState.currentError || "Error occurred")}
                {currentUiState === UI.BAR_STATES_SUCCESS && "Complete!"}
                {currentUiState === UI.BAR_STATES_SHRINKING && ""}
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
 *    - listen(EVENTS.BAR_STATE_UPDATE, handler)
 *    - Single source of truth from backend
 *    - Type-safe payload validation
 *
 * ✅ Command-Based User Interactions:
 *    - invoke("ui_handle_interaction", { elementId, interaction })
 *    - Standardized interaction event structure using UI.INTERACTION_TYPES_*
 *    - Comprehensive error handling
 *
 * ✅ Type Safety & Backend Alignment:
 *    - Types match backend exactly using UI.BAR_STATES_* constants
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
 *    - Reusable interaction patterns using constants
 *    - Comprehensive documentation
 *
 * This component serves as the reference implementation for all
 * floating UI components in the Juno application.
 */
