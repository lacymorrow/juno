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

// State mapping: BarState -> AssistantState
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

// Response content generator based on transcription/spoken text
const generateResponseContent = (
  transcriptionText: string,
  spokenText: string,
  currentError: string | null
): ResponseContent[] => {
  if (currentError) {
    return [
      {
        type: "text",
        title: "Error",
        content: currentError,
      },
    ];
  }

  if (spokenText) {
    return [
      {
        type: "text",
        title: "AI Response",
        content: spokenText,
      },
    ];
  }

  if (transcriptionText) {
    return [
      {
        type: "text",
        title: "Transcription",
        content: transcriptionText,
      },
    ];
  }

  return [];
};

// Custom VoiceAIBar wrapper with FloatingBar integration
interface IntegratedVoiceAIBarProps {
  assistantState: AssistantState;
  inputValue: string;
  sampleResponses: Record<string, ResponseContent>;
  onStateChange: (state: AssistantState) => void;
  onInputChange: (value: string) => void;
  onInputSubmit: (e: FormEvent) => void;
  onInputBlur: () => void;
  inputRef: React.RefObject<HTMLInputElement>;
  className?: string;
}

function IntegratedVoiceAIBar({
  assistantState,
  inputValue,
  sampleResponses,
  onStateChange,
  onInputChange,
  onInputSubmit,
  onInputBlur,
  inputRef,
  className,
}: IntegratedVoiceAIBarProps) {
  // Create a modified VoiceAIBar that uses our input handlers
  return (
    <div className={className}>
      <VoiceAIBar
        initialState={assistantState}
        onStateChange={onStateChange}
        sampleResponses={sampleResponses}
        className="integrated-voice-ai-bar"
      />

      {/* Overlay input handling for input state */}
      {assistantState === "input" && (
        <div className="absolute inset-0 flex items-center px-4">
          <form onSubmit={onInputSubmit} className="flex items-center gap-2 w-full">
            <input
              ref={inputRef}
              type="text"
              value={inputValue}
              onChange={(e) => onInputChange(e.target.value)}
              onBlur={onInputBlur}
              placeholder="Ask me anything..."
              className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
              autoFocus
            />
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={!inputValue.trim()}
            >
              <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z" />
              </svg>
            </button>
          </form>
        </div>
      )}
    </div>
  );
}

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

    // Generate response content for response state
    if (newAssistantState === "success" || newAssistantState === "error") {
      generateResponseContent(
        transcriptionText,
        spokenText,
        currentError
      );
    }
  }, [barState, assistantState, transcriptionText, spokenText, currentError]);

  // Update window size based on bar state
  useEffect(() => {
    const isCompact = ["default"].includes(barState);

    const targetSize = isCompact
      ? { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT }
      : { width: EXPANDED_WIDTH, height: EXPANDED_HEIGHT };

    if (isCompact) {
      setTimeout(() => {
        resizeWindow(targetSize);
      }, 1000);
    } else {
      resizeWindow(targetSize);
    }
  }, [barState, resizeWindow]);

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

  // VoiceAIBar state change handler
  const handleVoiceAIBarStateChange = useCallback(
    (newState: AssistantState) => {
      // This is called when VoiceAIBar internally changes state
      // We can use this to trigger backend actions if needed
      console.log("VoiceAIBar state changed to:", newState);

      // Handle specific state transitions that need backend interaction
      if (newState === "input") {
        handleBarClick(); // Trigger input mode in backend
      }
    },
    [handleBarClick]
  );

  // Sample responses for VoiceAIBar
  const sampleResponses = {
    text: {
      type: "text" as const,
      title: "Voice Assistant Response",
      content: spokenText || transcriptionText || "Voice assistant ready",
    },
    code: {
      type: "code" as const,
      title: "System Status",
      content: `Voice Mode: ${voiceMode}
Agent Working: ${isAgentWorking}
Dictation Mode: ${isDictationMode}
Always Listening: ${isAlwaysListening}
Audio Level: ${audioLevel}
Last Submitted: ${lastSubmittedValue || "None"}
Agent State: ${agentState || "Unknown"}`,
    },
    component: {
      type: "component" as const,
      title: "Error Details",
      content: currentError || "No errors",
    },
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
          <IntegratedVoiceAIBar
            assistantState={assistantState}
            inputValue={inputValue}
            sampleResponses={sampleResponses}
            onStateChange={handleVoiceAIBarStateChange}
            onInputChange={handleInputChange}
            onInputSubmit={handleSubmit}
            onInputBlur={handleInputBlur}
            inputRef={inputRef}
            className="floating-bar-voice-ai relative"
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
      `}</style>
    </div>
  );
}
