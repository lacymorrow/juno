import { useEffect, useState, useRef, useCallback, FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic, Zap, Volume2, MessageCircle, Keyboard, Send } from "lucide-react";
import { cn } from "@/lib/utils";
import { VoiceStatusIndicator } from "../VoiceStatusIndicator";
import { EVENTS, UI } from "@/lib/constants.generated";

// === STANDARDIZED UI API TYPES ===

/**
 * UI State enumeration - Uses generated constants from backend.
 * These values are emitted by the backend UIManager in BAR_STATE_UPDATE events.
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
 * Backend State Data Structure - Matches exactly what the backend emits.
 * This structure is defined in ui_commands.rs's emit_bar_state_update().
 */
interface BarStateData {
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  agentState: string | null;
}

/**
 * Standardized UI Interaction Event Structure.
 * This matches UIInteractionEvent in ui_commands.rs.
 */
interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

const COMPONENT_ID = "app-bar";

// === COMPONENT DEFINITION ===

const getMainIcon = (uiState: UIState) => {
  switch (uiState) {
    case UI.BAR_STATES_LISTENING:
      return <Mic size={14} className="text-blue-400" />;
    case UI.BAR_STATES_TRANSCRIBING:
      return <Mic size={14} className="animate-pulse text-blue-400" />;
    case UI.BAR_STATES_SPEAKING:
      return <Volume2 size={14} className="text-green-400" />;
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
      return <Zap size={14} className="animate-pulse text-yellow-400" />;
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
      return <MessageCircle size={14} className="text-white" />;
    case UI.BAR_STATES_DICTATION_READY:
      return <Keyboard size={14} className="text-orange-400" />;
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return <Mic size={14} className="text-blue-400 animate-pulse" />;
    default:
      return <Zap size={14} className="text-white" />;
  }
};

const getStatusText = (uiState: UIState, currentError: string | null) => {
  if (currentError) {
    return `Error: ${currentError}`;
  }

  switch (uiState) {
    case UI.BAR_STATES_DEFAULT:
      return "Click to start or use voice commands";
    case UI.BAR_STATES_EXPANDING:
      return "Preparing input field...";
    case UI.BAR_STATES_INPUT:
      return "Type your request or use voice input";
    case UI.BAR_STATES_SUBMITTING:
      return "Sending your request...";
    case UI.BAR_STATES_LOADING:
      return "Processing your request...";
    case UI.BAR_STATES_SPEAKING:
      return "Speaking response...";
    case UI.BAR_STATES_LISTENING:
      return "Listening for your voice...";
    case UI.BAR_STATES_TRANSCRIBING:
      return "Converting speech to text...";
    case UI.BAR_STATES_SUCCESS:
      return "Task completed successfully!";
    case UI.BAR_STATES_ERROR:
      return currentError || "An error occurred";
    case UI.BAR_STATES_FINISHING:
      return "Finalizing response...";
    case UI.BAR_STATES_DICTATION_READY:
      return "Ready for dictation mode";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "Always listening for wake words...";
    default:
      return "Ready";
  }
};

const AudioLevelIndicator = ({
  uiState,
  audioLevel,
}: {
  uiState: UIState;
  audioLevel: number;
}) => {
  if (
    ![
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_ALWAYS_LISTENING,
    ].includes(uiState as any)
  ) {
    return null;
  }

  const normalizedLevel = Math.min(Math.max(audioLevel * 100, 0), 100);
  const barCount = Math.ceil(normalizedLevel / 20); // 5 bars max

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

export function AppBar() {
  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    voiceMode: UI.VOICE_MODES_IDLE,
    audioLevel: 0,
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    agentState: null,
  });

  const [localInputValue, setLocalInputValue] = useState("");
  const [isWindowHovered] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let isCleanedUp = false;
    
    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            if (isCleanedUp) return; // Prevent updates after cleanup
            
            console.log("📨 AppBar: Received state update:", event.payload);
            const payload = event.payload;
            if (
              payload &&
              typeof payload === "object" &&
              "barState" in payload
            ) {
              setBarState(payload);
            } else {
              console.error(
                "❌ AppBar: Invalid state data received:",
                payload
              );
            }
          }
        );
        console.log("✅ AppBar: Event listener established");
      } catch (error) {
        console.error("❌ AppBar: Failed to setup event listener:", error);
      }
    };
    
    setupListener();
    
    return () => {
      isCleanedUp = true;
      if (unlisten) {
        try {
          unlisten();
          console.log("🔄 AppBar: Event listener cleaned up");
        } catch (error) {
          console.error("❌ AppBar: Error cleaning up listener:", error);
        }
      }
    };
  }, []);

  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  // === FOCUS MANAGEMENT ===
  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_INPUT && inputRef.current) {
      inputRef.current.focus();
    }
  }, [barState.barState]);

  // FIXME: This is a placeholder. A proper config system should be used.
  const uiConfig = {
    opacity: 0.95,
    showVoiceIndicator: true,
  };

  // === STANDARDIZED INTERACTION HANDLERS ===
  const createInteraction = useCallback(
    (
      interactionType: string,
      data?: Record<string, any>
    ): UIInteractionEvent => ({
      element_id: COMPONENT_ID,
      interaction_type: interactionType,
      data: data || null,
      timestamp: Date.now(),
    }),
    []
  );

  const sendInteraction = useCallback(
    async (interaction: UIInteractionEvent) => {
      try {
        console.log("🔧 AppBar: Sending interaction:", interaction);
        await invoke("ui_handle_interaction", {
          elementId: COMPONENT_ID,
          interaction,
        });
        console.log("✅ AppBar: Interaction sent successfully");
      } catch (error) {
        console.error("❌ AppBar: Interaction failed:", error);
      }
    },
    []
  );

  // === EVENT HANDLERS ===
  const handleBarClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
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
    [localInputValue, createInteraction, sendInteraction]
  );

  const handleInputFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS);
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const handleInputBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR);
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  // === STYLING ===
  const getContainerStyles = () => {
    let bgColor = "bg-black/90";

    switch (barState.voiceMode) {
      case UI.VOICE_MODES_DICTATION:
        bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
        break;
      case UI.VOICE_MODES_AGENT:
        bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
        break;
      default:
        if (barState.isDictationMode) {
          bgColor = "bg-gradient-to-r from-orange-600/98 to-orange-700/98";
        } else if (barState.isAgentWorking) {
          bgColor = "bg-gradient-to-r from-blue-600/98 to-blue-700/98";
        }
        break;
    }

    // Override for specific states
    if (barState.barState === UI.BAR_STATES_ERROR) {
      bgColor = "bg-gradient-to-r from-red-600/90 to-red-700/90";
    } else if (barState.barState === UI.BAR_STATES_SUCCESS) {
      bgColor = "bg-gradient-to-r from-emerald-600/90 to-emerald-700/90";
    } else if (barState.barState === UI.BAR_STATES_ALWAYS_LISTENING) {
      bgColor = "bg-gradient-to-r from-blue-500/98 to-cyan-600/98";
    }

    const sizeStyles = [UI.BAR_STATES_DEFAULT].includes(
      (barState.barState || UI.BAR_STATES_DEFAULT) as any
    )
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const hoverEffect =
      barState.barState === UI.BAR_STATES_DEFAULT && isWindowHovered
        ? "ring-2 ring-white/30"
        : "";

    const clickable = [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_DICTATION_READY,
    ].includes((barState.barState || UI.BAR_STATES_DEFAULT) as any)
      ? "cursor-pointer"
      : "";

    return cn(
      bgColor,
      sizeStyles,
      hoverEffect,
      clickable,
      "rounded-full backdrop-blur-xl border border-white/20 transition-all duration-300 ease-out shadow-lg"
    );
  };

  return (
    <div className="relative">
      <div
        className={getContainerStyles()}
        style={{ opacity: uiConfig.opacity }}
        onClick={
          [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
            (barState.barState || UI.BAR_STATES_DEFAULT) as any
          )
            ? handleBarClick
            : undefined
        }
      >
        {/* Default State */}
        {(barState.barState === UI.BAR_STATES_DEFAULT ||
          barState.barState === UI.BAR_STATES_DICTATION_READY ||
          barState.barState === UI.BAR_STATES_FINISHING) && (
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon(barState.barState || UI.BAR_STATES_DEFAULT)}
            {uiConfig.showVoiceIndicator &&
              (barState.voiceMode !== UI.VOICE_MODES_IDLE ||
                barState.isDictationMode ||
                barState.isAgentWorking) && (
                <VoiceStatusIndicator variant="compact" className="ml-1" />
              )}
            {barState.isAlwaysListening && (
              <div
                className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                data-tauri-drag-region
              />
            )}
          </div>
        )}

        {/* Input State */}
        {(barState.barState === UI.BAR_STATES_EXPANDING ||
          barState.barState === UI.BAR_STATES_INPUT) && (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3",
              "transition-opacity duration-300 ease-in-out",
              barState.barState === UI.BAR_STATES_INPUT
                ? "opacity-100"
                : "opacity-0"
            )}
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon(barState.barState || UI.BAR_STATES_INPUT)}
              <input
                ref={inputRef}
                type="text"
                value={localInputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onFocus={handleInputFocus}
                onBlur={handleInputBlur}
                placeholder="Ask me anything..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                disabled={barState.barState !== UI.BAR_STATES_INPUT}
              />
            </div>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={barState.barState !== UI.BAR_STATES_INPUT}
            >
              <Send size={14} />
            </button>
          </form>
        )}

        {/* Active States */}
        {[
          UI.BAR_STATES_SUBMITTING,
          UI.BAR_STATES_LOADING,
          UI.BAR_STATES_SPEAKING,
          UI.BAR_STATES_DICTATING,
          UI.BAR_STATES_TRANSCRIBING,
          UI.BAR_STATES_AGENT_RESPONDING,
          UI.BAR_STATES_LISTENING,
        ].includes((barState.barState || UI.BAR_STATES_DEFAULT) as any) && (
          <div
            className="flex items-center justify-between w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon(barState.barState || UI.BAR_STATES_DEFAULT)}
              <span
                className="text-sm font-medium truncate"
                data-tauri-drag-region
              >
                {getStatusText(
                  barState.barState || UI.BAR_STATES_DEFAULT,
                  barState.currentError
                )}
              </span>
            </div>
            <AudioLevelIndicator
              uiState={barState.barState || UI.BAR_STATES_DEFAULT}
              audioLevel={barState.audioLevel}
            />
          </div>
        )}

        {/* Other states would go here similar to FloatingBar... */}
      </div>
    </div>
  );
}
