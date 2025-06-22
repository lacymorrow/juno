import { Window } from "@tauri-apps/api/window";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import { VoiceAIBar } from "./bar/voice-ai-bar";
import { useInvoke } from "@/hooks/useInvoke";
import { useEventListener } from "@/hooks/useEventListener";
import { useWindowSize } from "@/hooks/useWindowSize";
import type {
  BarState,
  BarStateData,
  FloatingBarConfig,
  WindowConfig,
} from "@/types/floating-bar";
import type { AssistantState, ResponseContent } from "@/types/voice-ai";
import { FLOATING_BAR_DIMENSIONS } from "@/types/floating-bar";

// Get default window dimensions from tauri.conf.json
const floatingBarConfig = tauriConfig.app.windows.find(
  (window: WindowConfig) => window.label === "floating-bar"
);
const DEFAULT_WIDTH =
  floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
const DEFAULT_HEIGHT =
  floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

// Enhanced state mapping: BarState -> AssistantState
const mapBarStateToAssistantState = (barState: BarState): AssistantState => {
  switch (barState) {
    case "default":
    case "dictation_ready":
    case "finishing":
      return "idle";
    case "expanding":
    case "input":
      return "input";
    case "dictation_active":
    case "dictating":
    case "agent_listening":
    case "listening":
      return "listening";
    case "dictation_processing":
    case "transcribing":
    case "agent_thinking":
    case "loading":
      return "processing";
    case "agent_responding":
    case "speaking":
      return "speaking";
    case "success":
      return "success";
    case "error":
      return "error";
    case "always-listening":
      return "listening"; // Map to listening for now
    case "shrinking":
      return "idle";
    default:
      return "idle";
  }
};

// Generate comprehensive response content based on current state
const generateResponseContent = (
  transcriptionText: string,
  spokenText: string,
  currentError: string | null,
  voiceMode: string,
  isAgentWorking: boolean,
  isDictationMode: boolean,
  isAlwaysListening: boolean,
  audioLevel: number,
  lastSubmittedValue: string,
  agentState: string | null
): Record<string, ResponseContent> => {
  if (currentError) {
    return {
      error: {
        type: "text",
        title: "Error Details",
        content: currentError,
      },
    };
  }

  if (spokenText) {
    return {
      response: {
        type: "text",
        title: "AI Response",
        content: spokenText,
      },
    };
  }

  if (transcriptionText) {
    return {
      transcription: {
        type: "text",
        title: "Transcription",
        content: `"${transcriptionText}"`,
      },
    };
  }

  // System status for debugging/info
  return {
    status: {
      type: "code",
      title: "System Status",
      content: `Voice Mode: ${voiceMode}
Agent Working: ${isAgentWorking}
Dictation Mode: ${isDictationMode}
Always Listening: ${isAlwaysListening}
Audio Level: ${audioLevel.toFixed(2)}
Last Submitted: ${lastSubmittedValue || "None"}
Agent State: ${agentState || "Unknown"}`,
    },
  };
};

export function FloatingBar() {
  // Enhanced state management - mirrors backend state exactly
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [transcriptionText, setTranscriptionText] = useState("");
  const [spokenText, setSpokenText] = useState("");
  const [isAgentWorking, setIsAgentWorking] = useState(false);
  const [isDictationMode, setIsDictationMode] = useState(false);
  const [isAlwaysListening, setIsAlwaysListening] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [voiceMode, setVoiceMode] = useState<"dictation" | "agent" | "idle">(
    "idle"
  );
  const [agentState, setAgentState] = useState<string | null>(null);

  const { invokeCommand } = useInvoke();
  const { resizeWindow } = useWindowSize("floating-bar");

  // UI state
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const [isAnimatingSize, setIsAnimatingSize] = useState(false);
  const [showTooltip, setShowTooltip] = useState(false);
  const [config, setConfig] = useState<FloatingBarConfig>({
    showVoiceIndicator: true,
    enableAnimations: true,
    autoHide: false,
    autoHideDelay: 3000,
    opacity: 0.95,
  });

  // VoiceAIBar integration state
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");

  const inputRef = useRef<HTMLInputElement>(null);
  const tooltipTimeoutRef = useRef<NodeJS.Timeout>();

  // Load configuration from backend
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const savedConfig = await invokeCommand<FloatingBarConfig>(
          "get_floating_bar_config"
        );
        setConfig(savedConfig);
      } catch (error) {
        console.error("Failed to load floating bar config:", error);
      }
    };
    loadConfig();
  }, [invokeCommand]);

  // Update assistant state when bar state changes
  useEffect(() => {
    const newAssistantState = mapBarStateToAssistantState(barState);
    if (newAssistantState !== assistantState) {
      setAssistantState(newAssistantState);
    }
  }, [barState, assistantState]);

  // Dynamic window sizing based on assistant state and content
  useEffect(() => {
    const getWindowDimensionsForState = (state: AssistantState) => {
      switch (state) {
        case "idle":
          return { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT };
        case "input":
          return { width: 320, height: DEFAULT_HEIGHT };
        case "listening":
        case "processing":
        case "speaking":
          return { width: 280, height: DEFAULT_HEIGHT };
        case "response":
          // Dynamic sizing for response content
          return { width: EXPANDED_WIDTH, height: EXPANDED_HEIGHT };
        case "success":
        case "error":
          return { width: 240, height: DEFAULT_HEIGHT };
        default:
          return { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT };
      }
    };

    const targetSize = getWindowDimensionsForState(assistantState);

    // Smooth transition timing
    if (assistantState === "idle") {
      // Delay shrinking to allow animations to complete
      setTimeout(() => {
        resizeWindow(targetSize);
      }, 750);
    } else {
      // Immediate expansion for better UX
      resizeWindow(targetSize);
    }
  }, [assistantState, resizeWindow]);

  // Handle animation state tracking
  useEffect(() => {
    if (config.enableAnimations) {
      setIsAnimatingSize(["expanding", "shrinking"].includes(barState));
    }
  }, [barState, config.enableAnimations]);

  // Cleanup tooltip timeout on unmount
  useEffect(() => {
    return () => {
      if (tooltipTimeoutRef.current) {
        clearTimeout(tooltipTimeoutRef.current);
      }
    };
  }, []);

  // Listen for enhanced backend state updates
  const handleBarStateUpdate = useCallback((data: BarStateData) => {
    console.log("Received bar-state-update:", data);

    // Update all state from backend
    setBarState(data.barState);
    setInputValue(data.inputValue);
    setLastSubmittedValue(data.lastSubmittedValue);
    setCurrentError(data.currentError);
    setTranscriptionText(data.transcriptionText);
    setSpokenText(data.spokenText);
    setIsAgentWorking(data.isAgentWorking);
    setIsDictationMode(data.isDictationMode);
    setIsAlwaysListening(data.isAlwaysListening);
    setAudioLevel(data.audioLevel || 0);
    setVoiceMode(data.voiceMode || "idle");
    setAgentState(data.agentState || null);

    // Auto-focus input when in input state
    if (data.barState === "input" && inputRef.current) {
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
    }
  }, []);

  useEventListener("bar-state-update", handleBarStateUpdate);

  // Window hover handlers
  const handleMouseEnter = useCallback(() => {
    setIsWindowHovered(true);

    if (barState === "default") {
      setShowTooltip(true);
      if (tooltipTimeoutRef.current) {
        clearTimeout(tooltipTimeoutRef.current);
      }
      tooltipTimeoutRef.current = setTimeout(() => {
        setShowTooltip(false);
      }, 2000);
    }
  }, [barState]);

  const handleMouseLeave = useCallback(() => {
    setIsWindowHovered(false);
    setShowTooltip(false);
    if (tooltipTimeoutRef.current) {
      clearTimeout(tooltipTimeoutRef.current);
    }
  }, []);

  useEventListener("mouse-entered-window", handleMouseEnter, [barState]);
  useEventListener("mouse-left-window", handleMouseLeave);

  // Listen for window focus changes
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const currentWindow = Window.getCurrent();
        unlisten = await currentWindow.onFocusChanged(
          async ({ payload: isFocused }) => {
            console.log(
              "Window focus changed:",
              isFocused,
              "Current bar state:",
              barState
            );
            await invokeCommand("floating_bar_focus_change", { isFocused });
          }
        );
      } catch (error) {
        console.error("Failed to setup focus listener:", error);
      }
    };

    setupListener();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [barState, invokeCommand]);

  // Handler functions that call backend commands
  const handleBarClick = useCallback(async () => {
    await invokeCommand("floating_bar_click");
  }, [invokeCommand]);

  const handleInputBlur = useCallback(async () => {
    await invokeCommand("floating_bar_input_blur");
  }, [invokeCommand]);

  const handleInputChange = useCallback(
    async (value: string) => {
      setInputValue(value);
      await invokeCommand("floating_bar_input_change", { value });
    },
    [invokeCommand]
  );

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const query = inputValue.trim();
      if (!query) return;

      await invokeCommand("floating_bar_submit", { query });
    },
    [inputValue, invokeCommand]
  );

  // VoiceAIBar state change handler - bridges VoiceAIBar internal state to backend
  const handleVoiceAIBarStateChange = useCallback(
    async (newState: AssistantState) => {
      console.log("VoiceAIBar state changed to:", newState);

      // Handle specific state transitions that need backend interaction
      switch (newState) {
        case "input":
          await handleBarClick(); // Trigger input mode in backend
          break;
        case "idle":
          // Return to default state
          if (assistantState === "input") {
            await handleInputBlur();
          }
          break;
        case "listening":
          // Could trigger voice listening if not already active
          break;
        default:
          // Other states are handled by backend events
          break;
      }
    },
    [handleBarClick, handleInputBlur, assistantState]
  );

  // Generate dynamic response content based on current state
  const sampleResponses = generateResponseContent(
    transcriptionText,
    spokenText,
    currentError,
    voiceMode,
    isAgentWorking,
    isDictationMode,
    isAlwaysListening,
    audioLevel,
    lastSubmittedValue,
    agentState
  );

  // Custom input handling props for VoiceAIBar
  const inputHandlingProps = {
    inputValue,
    onInputChange: handleInputChange,
    onInputSubmit: handleSubmit,
    onInputBlur: handleInputBlur,
    inputRef,
  };

  // Use state variables to prevent unused warnings
  void isWindowHovered;
  void isAnimatingSize;

  return (
    <div
      data-tauri-drag-region
      className="w-screen h-screen flex items-start justify-start relative"
    >
      {/* Tooltip */}
      {showTooltip && barState === "default" && (
        <div
          className="absolute top-16 left-8 z-50 animate-fade-in pointer-events-none cursor-move"
          data-tauri-drag-region
        >
          <div
            className="bg-black/90 text-white text-xs px-3 py-2 rounded-lg border border-white/20 backdrop-blur-md max-w-xs cursor-move"
            data-tauri-drag-region
          >
            Voice assistant ready - {voiceMode} mode
          </div>
        </div>
      )}

      <div className="relative z-50 p-3 bg-transparent" data-tauri-drag-region>
        <div
          data-tauri-drag-region
          style={{ opacity: config.opacity }}
          onClick={
            ["default", "dictation_ready"].includes(barState)
              ? handleBarClick
              : undefined
          }
        >
          <VoiceAIBar
            initialState={assistantState}
            onStateChange={handleVoiceAIBarStateChange}
            sampleResponses={sampleResponses}
            className="floating-bar-voice-ai relative"
            // Pass input handling props to VoiceAIBar
            {...inputHandlingProps}
          />
        </div>

        {/* Voice Status Indicator - Show when enabled and relevant */}
        {config.showVoiceIndicator &&
          (voiceMode !== "idle" || isDictationMode || isAgentWorking) && (
            <div className="absolute top-2 right-2 z-60">
              <VoiceStatusIndicator variant="compact" className="opacity-75" />
            </div>
          )}

        {/* Always Listening Indicator */}
        {isAlwaysListening && (
          <div className="absolute bottom-1 right-1 z-60">
            <div className="w-2 h-2 bg-blue-400 rounded-full animate-pulse"></div>
          </div>
        )}

        {/* Audio Level Indicator for debugging */}
        {audioLevel > 0 && process.env.NODE_ENV === "development" && (
          <div className="absolute bottom-2 left-2 z-60">
            <div
              className="bg-green-400 h-1 rounded-full transition-all duration-150"
              style={{ width: `${Math.min(audioLevel * 20, 40)}px` }}
            />
          </div>
        )}
      </div>

      <style>{`
        .floating-bar-voice-ai {
          /* Ensure proper drag region handling */
        }
        .floating-bar-voice-ai * {
          cursor: move;
        }
        .floating-bar-voice-ai button {
          cursor: pointer;
        }
        .floating-bar-voice-ai input {
          cursor: text;
        }

        /* Custom animations for floating bar context */
        @keyframes fade-in {
          from {
            opacity: 0;
            transform: translateY(4px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        .animate-fade-in {
          animation: fade-in 0.3s cubic-bezier(0.4, 0, 0.2, 1) both;
        }

        /* Enhanced drag region support */
        [data-tauri-drag-region] {
          -webkit-app-region: drag;
        }

        [data-tauri-drag-region] button,
        [data-tauri-drag-region] input,
        [data-tauri-drag-region] [role="button"] {
          -webkit-app-region: no-drag;
        }
      `}</style>
    </div>
  );
}
