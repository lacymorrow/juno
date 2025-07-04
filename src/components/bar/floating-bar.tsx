"use client";

import { useEffect, useState, useCallback, type FormEvent } from "react";
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

// === MOCK CONSTANTS (Replace with actual imports) ===
const UI = {
  BAR_STATES_DEFAULT: "default",
  BAR_STATES_EXPANDING: "expanding",
  BAR_STATES_INPUT: "input",
  BAR_STATES_SHRINKING: "shrinking",
  BAR_STATES_SUBMITTING: "submitting",
  BAR_STATES_LOADING: "loading",
  BAR_STATES_FINISHING: "finishing",
  BAR_STATES_SUCCESS: "success",
  BAR_STATES_LISTENING: "listening",
  BAR_STATES_ERROR: "error",
  BAR_STATES_TRANSCRIBING: "transcribing",
  BAR_STATES_SPEAKING: "speaking",
  BAR_STATES_DICTATING: "dictating",
  BAR_STATES_DICTATION_READY: "dictation_ready",
  BAR_STATES_ALWAYS_LISTENING: "always_listening",
  BAR_STATES_AGENT_RESPONDING: "agent_responding",
  VOICE_MODES_IDLE: "idle",
  INTERACTION_TYPES_CLICK: "click",
  INTERACTION_TYPES_SUBMIT: "submit",
  INTERACTION_TYPES_FOCUS: "focus",
  INTERACTION_TYPES_BLUR: "blur",
} as const;

// === TYPES ===
type UIState = (typeof UI)[keyof typeof UI];

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

interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

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
  if (
    ![
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_ALWAYS_LISTENING,
    ].includes(currentUiState)
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

const FloatingBarContent = () => {
  const { setSize } = useDynamicIslandSize();

  // === STATE MANAGEMENT ===
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

  // === MOCK BACKEND INTEGRATION ===
  // In real implementation, replace with actual Tauri event listeners
  useEffect(() => {
    // Mock state changes for demo
    const mockStateSequence = [
      { state: UI.BAR_STATES_DEFAULT, delay: 1000 },
      { state: UI.BAR_STATES_EXPANDING, delay: 2000 },
      { state: UI.BAR_STATES_INPUT, delay: 3000 },
      { state: UI.BAR_STATES_LISTENING, delay: 5000 },
      { state: UI.BAR_STATES_TRANSCRIBING, delay: 6000 },
      { state: UI.BAR_STATES_LOADING, delay: 7000 },
      { state: UI.BAR_STATES_SUCCESS, delay: 8000 },
      { state: UI.BAR_STATES_DEFAULT, delay: 9000 },
    ];

    mockStateSequence.forEach(({ state, delay }) => {
      setTimeout(() => {
        setBarState((prev) => ({ ...prev, barState: state }));
      }, delay);
    });
  }, []);

  // === SYNC DYNAMIC ISLAND SIZE WITH STATE ===
  useEffect(() => {
    const newSize = getIslandSizeForState(barState.barState);
    setSize(newSize);
  }, [barState.barState, setSize]);

  // === INTERACTION HANDLERS ===
  const createInteraction = (
    interactionType: string,
    data?: Record<string, any>
  ): UIInteractionEvent => ({
    element_id: "floating-bar",
    interaction_type: interactionType,
    data: data || null,
    timestamp: Date.now(),
  });

  const sendInteraction = async (interaction: UIInteractionEvent) => {
    try {
      console.log("🔧 FloatingBar: Sending interaction:", interaction);
      // In real implementation: await invoke("ui_handle_interaction", { elementId: "floating-bar", interaction });
    } catch (error) {
      console.error("❌ FloatingBar: Interaction failed:", error);
    }
  };

  const handleClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
    // Mock: trigger expansion
    setBarState((prev) => ({ ...prev, barState: UI.BAR_STATES_EXPANDING }));
    setTimeout(() => {
      setBarState((prev) => ({ ...prev, barState: UI.BAR_STATES_INPUT }));
    }, 300);
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
        setBarState((prev) => ({
          ...prev,
          barState: UI.BAR_STATES_SUBMITTING,
        }));
      }
    },
    [localInputValue]
  );

  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
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
  const isCompact = [
    UI.BAR_STATES_DEFAULT,
    UI.BAR_STATES_DICTATION_READY,
  ].includes(currentUiState);

  return (
    <>
      {/* Compact States - Default and Dictation Ready */}
      {isCompact && (
        <DynamicContainer
          className="flex items-center justify-center h-full w-full cursor-pointer"
          onClick={handleClick}
        >
          <DynamicDiv className="flex items-center gap-2">
            {getMainIcon()}
            {barState.voiceMode !== UI.VOICE_MODES_IDLE && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          </DynamicDiv>
        </DynamicContainer>
      )}

      {/* Active States with Audio Feedback */}
      {[
        UI.BAR_STATES_LISTENING,
        UI.BAR_STATES_TRANSCRIBING,
        UI.BAR_STATES_SPEAKING,
        UI.BAR_STATES_DICTATING,
        UI.BAR_STATES_AGENT_RESPONDING,
      ].includes(currentUiState) && (
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
      {[
        UI.BAR_STATES_SUBMITTING,
        UI.BAR_STATES_LOADING,
        UI.BAR_STATES_ERROR,
        UI.BAR_STATES_SUCCESS,
        UI.BAR_STATES_SHRINKING,
        UI.BAR_STATES_FINISHING,
      ].includes(currentUiState) && (
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
      {[UI.BAR_STATES_ALWAYS_LISTENING].includes(currentUiState) && (
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
