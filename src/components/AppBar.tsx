import { useEffect, useState, useRef, useCallback, FormEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Mic,
  Zap,
  Volume2,
  MessageCircle,
  Keyboard,
  Send,
  AlertCircle,
  Check,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import type { BarState } from "@/types/floating-bar";

// === NEW UI API IMPORTS ===
import { useUIElement, UIState, type AgentStatus } from "@/lib/ui-api";

// === STATE CONVERSION UTILITIES ===

// Convert BarState to UI API UIState
const convertBarStateToUIState = (barState: BarState): UIState => {
  switch (barState) {
    case "default":
      return "default";
    case "expanding":
      return "expanding";
    case "input":
      return "input";
    case "submitting":
      return "submitting";
    case "loading":
      return "loading";
    case "speaking":
      return "speaking";
    case "listening":
      return "listening";
    case "transcribing":
      return "transcribing";
    case "success":
      return "success";
    case "error":
      return "error";
    case "finishing":
      return "finishing";
    case "dictation_ready":
      return "dictation-ready";
    case "always-listening":
      return "always-listening";
    default:
      return "default";
  }
};

// Convert UI API UIState back to BarState
const convertUIStateToBarState = (uiState: UIState): BarState => {
  switch (uiState) {
    case "default":
      return "default";
    case "expanding":
      return "expanding";
    case "expanded":
      return "input"; // Expanded maps to input for bars
    case "input":
      return "input";
    case "submitting":
      return "submitting";
    case "loading":
      return "loading";
    case "speaking":
      return "speaking";
    case "listening":
      return "listening";
    case "transcribing":
      return "transcribing";
    case "success":
      return "success";
    case "error":
      return "error";
    case "finishing":
      return "finishing";
    case "dictation-ready":
      return "dictation_ready";
    case "always-listening":
      return "always-listening";
    default:
      return "default";
  }
};

// === COMPONENT DEFINITION ===

const getMainIcon = (barState: BarState) => {
  switch (barState) {
    case "listening":
      return <Mic size={14} className="text-blue-400" />;
    case "transcribing":
      return <Mic size={14} className="animate-pulse text-blue-400" />;
    case "speaking":
      return <Volume2 size={14} className="text-green-400" />;
    case "loading":
    case "submitting":
      return <Zap size={14} className="animate-pulse text-yellow-400" />;
    case "input":
    case "expanding":
      return <MessageCircle size={14} className="text-white" />;
    case "dictation_ready":
      return <Keyboard size={14} className="text-orange-400" />;
    case "always-listening":
      return <Mic size={14} className="text-blue-400 animate-pulse" />;
    default:
      return <Zap size={14} className="text-white" />;
  }
};

const getStatusText = (barState: BarState, currentError: string | null) => {
  if (currentError) {
    return `Error: ${currentError}`;
  }

  switch (barState) {
    case "default":
      return "Click to start or use voice commands";
    case "expanding":
      return "Preparing input field...";
    case "input":
      return "Type your request or use voice input";
    case "submitting":
      return "Sending your request...";
    case "loading":
      return "Processing your request...";
    case "speaking":
      return "Speaking response...";
    case "listening":
      return "Listening for your voice...";
    case "transcribing":
      return "Converting speech to text...";
    case "success":
      return "Task completed successfully!";
    case "error":
      return currentError || "An error occurred";
    case "finishing":
      return "Finalizing response...";
    case "dictation_ready":
      return "Ready for dictation mode";
    case "always-listening":
      return "Always listening for wake words...";
    default:
      return "Ready";
  }
};

const AudioLevelIndicator = ({
  barState,
  audioLevel,
}: {
  barState: BarState;
  audioLevel: number;
}) => {
  if (!["listening", "transcribing", "always-listening"].includes(barState)) {
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
  // === UI API INTEGRATION ===
  const {
    manager,
    state,
    config,
    click,
    focus,
    blur,
    input,
    submit,
    updateState,
  } = useUIElement("floating-bar", "bar");

  // === LOCAL STATE ===
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [isWindowHovered] = useState(false);
  const [isAnimatingSize] = useState(false);
  const [showTooltip] = useState(false);
  const [currentError, setCurrentError] = useState<string | null>(null);

  // Context-like state (would normally come from context)
  const [voiceMode, setVoiceMode] = useState<"idle" | "agent" | "dictation">(
    "idle"
  );
  const [isDictationMode, setIsDictationMode] = useState(false);
  const [isAgentWorking, setIsAgentWorking] = useState(false);
  const [isAlwaysListening, setIsAlwaysListening] = useState(false);
  const [agentState, setAgentState] = useState<AgentStatus>("idle");
  const [audioLevel, setAudioLevel] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);

  // === SYNC UI API STATE WITH LOCAL STATE ===
  useEffect(() => {
    if (state) {
      console.log("AppBar: Syncing UI API state:", state);

      const newBarState = convertUIStateToBarState(state.uiState);
      setBarState(newBarState);
      setInputValue(state.inputValue);
      setCurrentError(state.currentError);
      setVoiceMode(state.voiceMode);
      setIsDictationMode(state.isDictationMode);
      setIsAgentWorking(state.isAgentWorking);
      setIsAlwaysListening(state.isAlwaysListening);
      setAgentState(state.agentState);
      setAudioLevel(state.audioLevel);
    }
  }, [state]);

  // === SYNC LOCAL STATE TO UI API ===
  useEffect(() => {
    if (manager) {
      const uiState = convertBarStateToUIState(barState);
      updateState({
        uiState,
        inputValue,
        currentError,
        voiceMode,
        isDictationMode,
        isAgentWorking,
        isAlwaysListening,
        agentState,
        audioLevel,
      });
    }
  }, [
    manager,
    updateState,
    barState,
    inputValue,
    currentError,
    voiceMode,
    isDictationMode,
    isAgentWorking,
    isAlwaysListening,
    agentState,
    audioLevel,
  ]);

  // === CONFIG LOADING ===
  // Note: State and config are automatically loaded by useUIElement hook

  // === FOCUS CHANGE HANDLER ===
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const currentWindow = getCurrentWindow();
        unlisten = await currentWindow.onFocusChanged(
          async ({ payload: isFocused }) => {
            console.log(
              "Window focus changed:",
              isFocused,
              "Current bar state:",
              barState
            );
            if (manager) {
              await focus({ isFocused });
            }
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
  }, [manager, focus, barState]);

  // === HANDLER FUNCTIONS USING UI API ===
  const handleBarClick = useCallback(async () => {
    if (manager) {
      await click();
    }
  }, [manager, click]);

  const handleInputBlur = useCallback(async () => {
    if (manager) {
      await blur();
    }
  }, [manager, blur]);

  const handleInputChange = useCallback(
    async (value: string) => {
      setInputValue(value);
      if (manager) {
        await input(value);
      }
    },
    [manager, input]
  );

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const query = inputValue.trim();
      if (!query) return;

      if (manager) {
        await submit(query);
      }
    },
    [inputValue, manager, submit]
  );

  // === UI LOGIC ===

  // Focus input when entering input state
  useEffect(() => {
    if (barState === "input" && inputRef.current) {
      setTimeout(() => {
        inputRef.current?.focus();
      }, 100);
    }
  }, [barState]);

  const floatingBarConfig = config || {
    opacity: 0.95,
    showVoiceIndicator: true,
  };

  // Get enhanced container styles with voice mode awareness
  const getContainerStyles = () => {
    const baseStyles = `
      relative flex items-center justify-center
      text-white rounded-full shadow-lg border border-white/20
      transition-all duration-300 ease-in-out
      [will-change:width,height,transform]
      [backface-visibility:hidden]
      [transform-origin:center]
			cursor-move
    `;

    // Enhanced background based on voice mode and state
    let bgColor = "bg-black/90";

    switch (voiceMode) {
      case "dictation":
        bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
        break;
      case "agent":
        bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
        break;
      default:
        if (isDictationMode) {
          bgColor = "bg-gradient-to-r from-orange-600/98 to-orange-700/98";
        } else if (isAgentWorking) {
          bgColor = "bg-gradient-to-r from-blue-600/98 to-blue-700/98";
        }
        break;
    }

    // Override for specific states
    if (barState === "error") {
      bgColor = "bg-gradient-to-r from-red-600/90 to-red-700/90";
    } else if (barState === "success") {
      bgColor = "bg-gradient-to-r from-emerald-600/90 to-emerald-700/90";
    } else if (barState === "always-listening") {
      bgColor = "bg-gradient-to-r from-blue-500/98 to-cyan-600/98";
    }

    const sizeStyles = ["default"].includes(barState)
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const hoverEffect =
      barState === "default" && isWindowHovered
        ? "[transform:scale3d(1.05,1.05,1)]"
        : "";

    const clickable = ["default", "dictation_ready"].includes(barState)
      ? "cursor-pointer"
      : "";

    return cn(
      baseStyles,
      bgColor,
      sizeStyles,
      hoverEffect,
      clickable,
      !isAnimatingSize && "backdrop-blur-md"
    );
  };

  // UI API handles loading and error states internally

  return (
    <div
      data-tauri-drag-region
      className="w-screen h-screen flex items-start justify-start relative overflow-hidden"
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
            {getStatusText(barState, currentError)}
          </div>
        </div>
      )}

      <div className="relative z-50 p-3 bg-transparent" data-tauri-drag-region>
        <div
          data-tauri-drag-region
          className={getContainerStyles()}
          style={{ opacity: floatingBarConfig.opacity }}
          onClick={
            ["default", "dictation_ready"].includes(barState)
              ? handleBarClick
              : undefined
          }
        >
          {/* Default State */}
          {(barState === "default" ||
            barState === "dictation_ready" ||
            barState === "finishing") && (
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon(barState)}
              {floatingBarConfig.showVoiceIndicator &&
                (voiceMode !== "idle" || isDictationMode || isAgentWorking) && (
                  <VoiceStatusIndicator variant="compact" className="ml-1" />
                )}
              {isAlwaysListening && (
                <div
                  className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                  data-tauri-drag-region
                ></div>
              )}
            </div>
          )}

          {/* Expanding/Input State */}
          {(barState === "expanding" || barState === "input") && (
            <form
              data-tauri-drag-region
              onSubmit={handleSubmit}
              className={cn(
                "flex items-center justify-between w-full h-full gap-3",
                "transition-opacity duration-300 ease-in-out",
                barState === "input" ? "opacity-100" : "opacity-0"
              )}
            >
              <div
                className="flex items-center gap-2 flex-1"
                data-tauri-drag-region
              >
                {getMainIcon(barState)}
                <input
                  ref={inputRef}
                  type="text"
                  value={inputValue}
                  onChange={(e) => handleInputChange(e.target.value)}
                  onBlur={handleInputBlur}
                  placeholder="Ask me anything..."
                  className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                  disabled={barState !== "input"}
                />
              </div>
              <button
                data-tauri-drag-region
                type="submit"
                className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
                disabled={barState !== "input"}
              >
                <Send size={14} />
              </button>
            </form>
          )}

          {/* Enhanced Voice States */}
          {[
            "dictating",
            "transcribing",
            "agent_responding",
            "listening",
          ].includes(barState) && (
            <div
              className="flex items-center justify-between w-full h-full"
              data-tauri-drag-region
            >
              <div
                className="flex items-center gap-3 flex-1 min-w-0"
                data-tauri-drag-region
              >
                {getMainIcon(barState)}
                <div className="flex-1 min-w-0" data-tauri-drag-region>
                  <div
                    className="text-sm font-medium truncate"
                    data-tauri-drag-region
                  >
                    {getStatusText(barState, currentError)}
                  </div>
                </div>
              </div>
              <AudioLevelIndicator
                barState={barState}
                audioLevel={audioLevel}
              />
            </div>
          )}

          {/* Always Listening State */}
          {barState === "always-listening" && (
            <div
              className="flex items-center justify-between w-full h-full"
              data-tauri-drag-region
            >
              <div
                className="flex items-center gap-3 flex-1 min-w-0"
                data-tauri-drag-region
              >
                <Mic className="h-4 w-4 text-blue-400 animate-pulse" />
                <span
                  className="text-sm text-blue-200 truncate font-medium"
                  data-tauri-drag-region
                >
                  Always listening for wake words...
                </span>
              </div>
              <div
                className="flex items-center gap-1 ml-2"
                data-tauri-drag-region
              >
                <div
                  className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                  data-tauri-drag-region
                />
                <div
                  className="w-1 h-2 bg-blue-300 rounded-full animate-pulse"
                  style={{ animationDelay: "0.1s" }}
                  data-tauri-drag-region
                />
                <div
                  className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                  style={{ animationDelay: "0.2s" }}
                  data-tauri-drag-region
                />
              </div>
            </div>
          )}

          {/* Speaking State */}
          {barState === "speaking" && (
            <div
              className="flex items-center justify-between w-full h-full"
              data-tauri-drag-region
            >
              <div
                className="flex items-center gap-3 flex-1 min-w-0"
                data-tauri-drag-region
              >
                <Volume2 className="h-4 w-4 text-purple-300 animate-pulse" />
                <span
                  className="text-sm text-white/90 truncate"
                  data-tauri-drag-region
                >
                  {agentState === "failed"
                    ? "Task failed"
                    : agentState === "cancelled"
                    ? "Task cancelled"
                    : agentState === "offline"
                    ? "Connection unavailable"
                    : "Playing AI response"}
                </span>
              </div>
            </div>
          )}

          {/* Submitting State */}
          {barState === "submitting" && (
            <div
              className="flex flex-col items-center justify-center w-full h-full gap-2"
              data-tauri-drag-region
            >
              <div className="flex items-center gap-2" data-tauri-drag-region>
                <Zap className="h-4 w-4 animate-spin text-yellow-400" />
                <span className="text-sm font-medium" data-tauri-drag-region>
                  Submitting
                </span>
              </div>
            </div>
          )}

          {/* Loading State */}
          {barState === "loading" && (
            <div
              className="flex flex-col items-center justify-center w-full h-full gap-2"
              data-tauri-drag-region
            >
              <div className="flex items-center gap-2" data-tauri-drag-region>
                <Zap className="h-4 w-4 animate-spin" />
                <span className="text-sm font-medium" data-tauri-drag-region>
                  Processing
                </span>
              </div>
            </div>
          )}

          {/* Success State */}
          {barState === "success" && (
            <div
              className="flex items-center justify-between w-full h-full animate-success-fade"
              data-tauri-drag-region
            >
              <div
                className="flex items-center gap-3 flex-1 min-w-0"
                data-tauri-drag-region
              >
                {agentState === "failed" ? (
                  <AlertCircle className="h-4 w-4 text-red-300" />
                ) : agentState === "cancelled" ? (
                  <X className="h-4 w-4 text-yellow-300" />
                ) : agentState === "offline" ? (
                  <AlertCircle className="h-4 w-4 text-orange-300" />
                ) : (
                  <Check className="h-4 w-4 text-emerald-300" />
                )}
                <span
                  className={cn(
                    "text-sm font-medium truncate",
                    agentState === "failed"
                      ? "text-red-100"
                      : agentState === "cancelled"
                      ? "text-yellow-100"
                      : agentState === "offline"
                      ? "text-orange-100"
                      : "text-emerald-100"
                  )}
                  data-tauri-drag-region
                >
                  {getStatusText(barState, currentError)}
                </span>
              </div>
              <div
                className={cn(
                  "flex items-center justify-center h-6 w-6 rounded-full",
                  agentState === "failed"
                    ? "bg-red-400"
                    : agentState === "cancelled"
                    ? "bg-yellow-400"
                    : agentState === "offline"
                    ? "bg-orange-400"
                    : "bg-emerald-400"
                )}
                data-tauri-drag-region
              >
                {agentState === "failed" ? (
                  <X size={12} className="text-red-900" />
                ) : agentState === "cancelled" ? (
                  <X size={12} className="text-yellow-900" />
                ) : agentState === "offline" ? (
                  <AlertCircle size={12} className="text-orange-900" />
                ) : (
                  <Check size={12} className="text-emerald-900" />
                )}
              </div>
            </div>
          )}

          {/* Error State */}
          {barState === "error" && (
            <div
              className="flex items-center justify-between w-full h-full"
              data-tauri-drag-region
            >
              <div
                className="flex items-center gap-3 flex-1 min-w-0"
                data-tauri-drag-region
              >
                <AlertCircle className="h-4 w-4 text-red-300" />
                <span
                  className="text-sm font-medium text-red-100 truncate"
                  data-tauri-drag-region
                >
                  {currentError || "Error occurred"}
                </span>
              </div>
              <div
                className="flex items-center justify-center h-6 w-6 rounded-full bg-red-400"
                data-tauri-drag-region
              >
                <X size={12} className="text-red-900" />
              </div>
            </div>
          )}

          {/* Shrinking State */}
          {barState === "shrinking" && (
            <div
              className="opacity-0 w-full h-full transition-opacity duration-300"
              data-tauri-drag-region
            >
              {getMainIcon(barState)}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
