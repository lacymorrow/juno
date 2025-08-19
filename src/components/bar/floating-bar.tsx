"use client";

import { useEffect, useState, useCallback, type FormEvent } from "react";
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
import {
  DynamicContainer,
  DynamicDescription,
  DynamicDiv,
  DynamicIsland,
  DynamicIslandProvider,
  DynamicTitle,
  type SizePresets,
  useDynamicIslandSize,
} from "@/components/ui/dynamic-island";
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../../src-tauri/tauri.conf.json";

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
const COMPONENT_ID = "floating-bar-dynamic";

// === STATE TO SIZE MAPPING ===
const getIslandSizeForState = (state: UIState): SizePresets => {
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
      return "default";
    case UI.BAR_STATES_EXPANDING:
      return "compact";
    case UI.BAR_STATES_INPUT:
      return "long";
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_SPEAKING:
      return "compactLong";
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
      return "compact";
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
      return "compactMedium";
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "medium";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "large";
    default:
      return "default";
  }
};

// === COMPONENTS ===

const VoiceStatusIndicator = ({
  variant,
  className,
}: {
  variant: string;
  className?: string;
}) => (
  <div className={cn("flex items-center gap-1", className)}>
    <div className="w-2 h-2 bg-blue-400 rounded-full animate-pulse" />
    {variant !== "compact" && (
      <span className="text-xs text-blue-400">Voice</span>
    )}
  </div>
);

const AudioLevelIndicator = ({
  audioLevel,
  currentUiState,
}: {
  audioLevel: number;
  currentUiState: UIState;
}) => {
  const validStates = [
    UI.BAR_STATES_LISTENING,
    UI.BAR_STATES_TRANSCRIBING,
    UI.BAR_STATES_ALWAYS_LISTENING,
  ];

  if (!validStates.includes(currentUiState as any)) {
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

const FloatingBarContent = () => {
  const { setSize } = useDynamicIslandSize();

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

  const [localInputValue, setLocalInputValue] = useState("");

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
   * Primary backend integration: Listen to BAR_STATE_UPDATE events
   * This is the core pattern for all UI components - event-driven state updates
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            console.log(
              "📨 FloatingBar: Received state update:",
              event.payload
            );

            // Validate the received data structure
            const payload = event.payload;
            if (
              payload &&
              typeof payload === "object" &&
              "barState" in payload
            ) {
              setBarState(payload);
            } else {
              console.error(
                "❌ FloatingBar: Invalid state data received:",
                payload
              );
            }
          }
        );

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

  // === SYNC DYNAMIC ISLAND SIZE WITH STATE ===
  useEffect(() => {
    const newSize = getIslandSizeForState(barState.barState);
    setSize(newSize);
  }, [barState.barState, setSize]);

  // === WINDOW RESIZING LOGIC ===

  /**
   * Responsive window resizing based on UI state
   */
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const currentUiState = barState.barState;

        // Define compact states that use small window size
        const isCompact = [
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_LISTENING,
          UI.BAR_STATES_DICTATION_READY,
          UI.BAR_STATES_SPEAKING,
          UI.BAR_STATES_TRANSCRIBING,
        ].includes(currentUiState as any);
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
    }
  };

  /**
   * Sync local input state with backend state updates
   */
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  const handleClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, []);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const trimmedValue = localInputValue.trim();

      if (trimmedValue) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_SUBMIT, {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      }
    },
    [localInputValue]
  );

  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
  }, []);

  const handleFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS);
    await sendInteraction(interaction);
  }, []);

  const handleBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR);
    await sendInteraction(interaction);
  }, []);

  // === VISUAL HELPERS ===
  const getMainIcon = () => {
    switch (barState.barState) {
      case UI.BAR_STATES_LISTENING:
        return <Mic size={16} className="text-blue-400" />;
      case UI.BAR_STATES_TRANSCRIBING:
        return <Mic size={16} className="animate-pulse text-blue-400" />;
      case UI.BAR_STATES_SPEAKING:
        return <Volume2 size={16} className="text-green-400" />;
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return <Loader2 size={16} className="animate-spin text-yellow-400" />;
      case UI.BAR_STATES_INPUT:
      case UI.BAR_STATES_EXPANDING:
        return <Sparkles size={16} className="text-white" />;
      case UI.BAR_STATES_DICTATION_READY:
        return <Type size={16} className="text-orange-400" />;
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return <Mic size={16} className="text-blue-400 animate-pulse" />;
      case UI.BAR_STATES_ERROR:
        return <AlertCircle size={16} className="text-red-400" />;
      case UI.BAR_STATES_SUCCESS:
        return <Check size={16} className="text-green-400" />;
      default:
        return <Brain size={16} className="text-white" />;
    }
  };

  const getStateText = () => {
    switch (barState.barState) {
      case UI.BAR_STATES_LISTENING:
        return "Listening...";
      case UI.BAR_STATES_TRANSCRIBING:
        return "Converting speech...";
      case UI.BAR_STATES_SPEAKING:
        return "Playing response...";
      case UI.BAR_STATES_DICTATING:
        return "Dictating text...";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return "Agent working...";
      case UI.BAR_STATES_SUBMITTING:
        return "Sending...";
      case UI.BAR_STATES_LOADING:
        return "Processing...";
      case UI.BAR_STATES_FINISHING:
        return "Finishing...";
      case UI.BAR_STATES_ERROR:
        return barState.currentError || "Error occurred";
      case UI.BAR_STATES_SUCCESS:
        return "Complete!";
      default:
        return "";
    }
  };

  // === RENDER LOGIC ===
  const currentUiState = barState.barState;
  const compactStates = [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY];
  const isCompact = compactStates.includes(currentUiState as any);

  return (
    <>
      {/* Compact States - Default and Dictation Ready */}
      {isCompact && (
        <DynamicContainer className="flex items-center justify-center h-full w-full">
          <button
            type="button"
            className="flex items-center gap-2 cursor-pointer bg-transparent p-0 m-0 border-0"
            onClick={handleClick}
            aria-label="Activate assistant"
          >
            {getMainIcon()}
            {barState.voiceMode !== UI.VOICE_MODES_IDLE && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          </button>
        </DynamicContainer>
      )}

      {/* Active States with Audio Feedback */}
      {(() => {
        const activeStates = [
          UI.BAR_STATES_LISTENING,
          UI.BAR_STATES_TRANSCRIBING,
          UI.BAR_STATES_SPEAKING,
          UI.BAR_STATES_DICTATING,
          UI.BAR_STATES_AGENT_RESPONDING,
        ];
        return activeStates.includes(currentUiState as any);
      })() && (
        <DynamicContainer className="flex items-center justify-between w-full h-full px-4">
          <DynamicDiv className="flex items-center gap-3">
            {getMainIcon()}
            <DynamicDescription className="text-sm font-medium text-white">
              {getStateText()}
            </DynamicDescription>
          </DynamicDiv>
          <AudioLevelIndicator
            audioLevel={barState.audioLevel}
            currentUiState={currentUiState}
          />
        </DynamicContainer>
      )}

      {/* Input State - Interactive Form */}
      {(currentUiState === UI.BAR_STATES_EXPANDING ||
        currentUiState === UI.BAR_STATES_INPUT) && (
        <DynamicContainer className="w-full h-full">
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3 px-4",
              "transition-opacity duration-300 ease-in-out",
              currentUiState === UI.BAR_STATES_INPUT
                ? "opacity-100"
                : "opacity-0"
            )}
          >
            <DynamicDiv className="flex items-center gap-3 flex-1">
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
                autoFocus={currentUiState === UI.BAR_STATES_INPUT}
              />
            </DynamicDiv>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={currentUiState !== UI.BAR_STATES_INPUT}
            >
              <Send size={14} />
            </button>
          </form>
        </DynamicContainer>
      )}

      {/* Status States - Loading, Error, Success */}
      {(() => {
        const statusStates = [
          UI.BAR_STATES_SUBMITTING,
          UI.BAR_STATES_LOADING,
          UI.BAR_STATES_ERROR,
          UI.BAR_STATES_SUCCESS,
          UI.BAR_STATES_SHRINKING,
          UI.BAR_STATES_FINISHING,
        ];
        return statusStates.includes(currentUiState as any);
      })() && (
        <DynamicContainer className="flex items-center justify-center w-full h-full">
          <DynamicDiv className="flex items-center gap-3">
            {getMainIcon()}
            <DynamicDescription className="text-sm font-medium text-white">
              {getStateText()}
            </DynamicDescription>
          </DynamicDiv>
        </DynamicContainer>
      )}

      {/* Complex States - Medium/Large layouts */}
      {currentUiState === UI.BAR_STATES_ALWAYS_LISTENING && (
        <DynamicContainer className="flex flex-col justify-center items-center w-full h-full p-4 text-center">
          <DynamicDiv className="flex items-center gap-3 mb-2">
            {getMainIcon()}
            <DynamicTitle className="text-lg font-bold text-white">
              Always Listening
            </DynamicTitle>
          </DynamicDiv>
          <DynamicDescription className="text-sm text-white/70">
            Voice commands are active
          </DynamicDescription>
          <AudioLevelIndicator
            audioLevel={barState.audioLevel}
            currentUiState={currentUiState}
          />
        </DynamicContainer>
      )}
    </>
  );
};

export function FloatingBar() {
  return (
    <DynamicIslandProvider initialSize="default">
      <div className="flex min-h-screen items-center justify-center bg-gray-900">
        <DynamicIsland id="floating-bar-refactored">
          <FloatingBarContent />
        </DynamicIsland>
      </div>
    </DynamicIslandProvider>
  );
}
