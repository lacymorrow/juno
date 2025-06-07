import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import {
  AlertCircle,
  Brain,
  Check,
  Loader2,
  Mic,
  Send,
  Sparkles,
  Type,
  Volume2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";

// Use the existing BarState type from the backend
type BarState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "loading"
  | "finishing"
  | "success"
  | "listening"
  | "error"
  | "transcribing"
  | "speaking"
  | "dictating";

// Use the existing BarStateData interface from the backend
interface BarStateData {
  barState: BarState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  isAgentWorking: boolean;
  isDictationMode: boolean;
}

interface FloatingBarConfig {
  showVoiceIndicator: boolean;
  enableAnimations: boolean;
  autoHide: boolean;
  autoHideDelay: number;
  opacity: number;
}

export function EnhancedFloatingBar() {
  // State management - using the existing backend state structure
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [transcriptionText, setTranscriptionText] = useState("");
  const [spokenText, setSpokenText] = useState("");
  const [isAgentWorking, setIsAgentWorking] = useState(false);
  const [isDictationMode, setIsDictationMode] = useState(false);

  // UI state
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const [isAnimatingSize, setIsAnimatingSize] = useState(false);
  const [showTooltip, setShowTooltip] = useState(false);
  const [config] = useState<FloatingBarConfig>({
    showVoiceIndicator: true,
    enableAnimations: true,
    autoHide: false,
    autoHideDelay: 3000,
    opacity: 0.95,
  });

  const inputRef = useRef<HTMLInputElement>(null);
  const tooltipTimeoutRef = useRef<NodeJS.Timeout>();

  // Window dimensions
  const DEFAULT_WIDTH = 110;
  const DEFAULT_HEIGHT = 60;
  const EXPANDED_WIDTH = 320;
  const EXPANDED_HEIGHT = 80;

  // Load configuration - removed non-existent backend call
  // Using default config values for now

  // Update window size based on bar state
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = await Window.getByLabel("floating-bar");
        const isExpanded = !["default", "shrinking", "finishing"].includes(
          barState
        );

        if (isExpanded) {
          await appWindow?.setSize(
            new LogicalSize(EXPANDED_WIDTH, EXPANDED_HEIGHT)
          );
        } else {
          await appWindow?.setSize(
            new LogicalSize(DEFAULT_WIDTH, DEFAULT_HEIGHT)
          );
        }
      } catch (err) {
        console.error("Failed to resize window:", err);
      }
    };
    resizeWindow();
  }, [barState]);

  // Handle animation state tracking
  useEffect(() => {
    if (config.enableAnimations) {
      setIsAnimatingSize(["expanding", "shrinking"].includes(barState));
    }
  }, [barState, config.enableAnimations]);

  // Listen for backend state updates - using the existing event name
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<BarStateData>(
        "bar-state-update", // Using existing event name
        (event) => {
          console.log("Received bar-state-update:", event.payload);
          const data = event.payload;

          // Update all state from backend
          setBarState(data.barState);
          setInputValue(data.inputValue);
          setLastSubmittedValue(data.lastSubmittedValue);
          setCurrentError(data.currentError);
          setTranscriptionText(data.transcriptionText);
          setSpokenText(data.spokenText);
          setIsAgentWorking(data.isAgentWorking);
          setIsDictationMode(data.isDictationMode);

          // Auto-focus input when in input state
          if (data.barState === "input" && inputRef.current) {
            requestAnimationFrame(() => {
              inputRef.current?.focus();
            });
          }
        }
      );
    };

    setupListener();
    return () => unlisten?.();
  }, []);

  // Listen for window hover events
  useEffect(() => {
    let unlistenEnter: (() => void) | undefined;
    let unlistenLeave: (() => void) | undefined;

    const setupListeners = async () => {
      unlistenEnter = await listen<null>("mouse-entered-window", () => {
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
      });

      unlistenLeave = await listen<null>("mouse-left-window", () => {
        setIsWindowHovered(false);
        setShowTooltip(false);
        if (tooltipTimeoutRef.current) {
          clearTimeout(tooltipTimeoutRef.current);
        }
      });
    };

    setupListeners();
    return () => {
      unlistenEnter?.();
      unlistenLeave?.();
    };
  }, [barState]);

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
            try {
              await invoke("floating_bar_focus_change", { isFocused });
            } catch (err) {
              console.error("Failed to handle focus change:", err);
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
  }, [barState]);

  // Handler functions - using existing backend function names
  const handleBarClick = async () => {
    try {
      await invoke("floating_bar_click"); // Using existing function name
    } catch (err) {
      console.error("Failed to handle bar click:", err);
    }
  };

  const handleInputBlur = async () => {
    try {
      await invoke("floating_bar_input_blur");
    } catch (err) {
      console.error("Failed to handle input blur:", err);
    }
  };

  const handleInputChange = async (value: string) => {
    try {
      await invoke("floating_bar_input_change", { value }); // Using existing function name
    } catch (err) {
      console.error("Failed to handle input change:", err);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const query = inputValue.trim();
    if (!query) return;

    try {
      await invoke("floating_bar_submit", { query }); // Using existing function name
    } catch (err) {
      console.error("Failed to handle submit:", err);
    }
  };

  // Get main icon based on state - enhanced with better state mapping
  const getMainIcon = () => {
    switch (barState) {
      case "default":
        return <Sparkles className="h-4 w-4 text-emerald-400" />;
      case "dictating":
        return <Type className="h-4 w-4 text-orange-500" />;
      case "listening":
        return <Brain className="h-4 w-4 text-blue-500" />;
      case "transcribing":
        return <Loader2 className="h-4 w-4 text-orange-500 animate-spin" />;
      case "speaking":
        return <Volume2 className="h-4 w-4 text-purple-500" />;
      case "loading":
        return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
      case "success":
        return <Check className="h-4 w-4 text-emerald-500" />;
      case "error":
        return <AlertCircle className="h-4 w-4 text-red-500" />;
      default:
        return <Mic className="h-4 w-4 text-blue-500" />;
    }
  };

  // Get status text for tooltip
  const getStatusText = () => {
    switch (barState) {
      case "default":
        return "Click to interact • Option+D for AI • Hold Space to dictate";
      case "dictating":
        return "Dictating... Release key to finish";
      case "transcribing":
        return "Processing dictation...";
      case "listening":
        return "Listening for voice command...";
      case "speaking":
        return "Playing AI response";
      case "loading":
        return "Processing request...";
      case "success":
        return "Task completed successfully";
      case "error":
        return currentError || "An error occurred";
      default:
        return "Voice assistant ready";
    }
  };

  // Get container styles with enhanced visual feedback
  const getContainerStyles = () => {
    const baseStyles = `
      relative flex items-center justify-center
      text-white rounded-full shadow-lg border border-white/20
      transition-all duration-300 ease-in-out
      [will-change:width,height,transform]
      [backface-visibility:hidden]
      [transform-origin:center]
    `;

    // Background based on state with gradients for better visual feedback
    let bgColor = "bg-black/90";

    if (isDictationMode) {
      bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
    } else if (isAgentWorking) {
      bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
    }

    // Override for specific states
    if (barState === "error") {
      bgColor = "bg-gradient-to-r from-red-600/90 to-red-700/90";
    } else if (barState === "success") {
      bgColor = "bg-gradient-to-r from-emerald-600/90 to-emerald-700/90";
    } else if (barState === "dictating") {
      bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
    } else if (barState === "listening") {
      bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
    }

    const sizeStyles = ["default", "shrinking", "finishing"].includes(barState)
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const hoverEffect =
      barState === "default" && isWindowHovered
        ? "[transform:scale3d(1.05,1.05,1)]"
        : "";

    const clickable = ["default"].includes(barState) ? "cursor-pointer" : "";

    return cn(
      baseStyles,
      bgColor,
      sizeStyles,
      hoverEffect,
      clickable,
      !isAnimatingSize && "backdrop-blur-md"
    );
  };

  return (
    <div className="w-screen h-screen flex items-start justify-start relative">
      {/* Enhanced Tooltip */}
      {showTooltip && barState === "default" && (
        <div className="absolute top-16 left-8 z-50 animate-fade-in">
          <div className="bg-black/90 text-white text-xs px-3 py-2 rounded-lg border border-white/20 backdrop-blur-md max-w-xs">
            {getStatusText()}
          </div>
        </div>
      )}

      <div className="relative z-50 p-3">
        <div
          data-tauri-drag-region
          className={getContainerStyles()}
          style={{ opacity: config.opacity }}
          onClick={barState === "default" ? handleBarClick : undefined}
        >
          {/* Default State */}
          {(barState === "default" || barState === "finishing") && (
            <div className="flex items-center gap-2">
              {getMainIcon()}
              {config.showVoiceIndicator &&
                (isDictationMode || isAgentWorking) && (
                  <VoiceStatusIndicator variant="compact" className="ml-1" />
                )}
            </div>
          )}

          {/* Expanding/Input State */}
          {(barState === "expanding" || barState === "input") && (
            <form
              onSubmit={handleSubmit}
              className={cn(
                "flex items-center justify-between w-full h-full gap-3",
                "transition-opacity duration-300 ease-in-out",
                barState === "input" ? "opacity-100" : "opacity-0"
              )}
            >
              <div className="flex items-center gap-2 flex-1">
                {getMainIcon()}
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
                type="submit"
                className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
                disabled={barState !== "input"}
              >
                <Send size={14} />
              </button>
            </form>
          )}

          {/* Voice States - Enhanced with better visual feedback */}
          {["dictating", "transcribing", "listening"].includes(barState) && (
            <div className="flex items-center justify-between w-full h-full">
              <div className="flex items-center gap-3 flex-1 min-w-0">
                {getMainIcon()}
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">
                    {getStatusText()}
                  </div>
                  {transcriptionText && (
                    <div className="text-xs text-white/70 truncate">
                      "{transcriptionText}"
                    </div>
                  )}
                </div>
              </div>
              {(barState === "dictating" || barState === "listening") && (
                <div className="flex items-center gap-1 ml-2">
                  <div className="w-1 h-2 bg-white/60 rounded-full animate-pulse" />
                  <div
                    className="w-1 h-3 bg-white/80 rounded-full animate-pulse"
                    style={{ animationDelay: "0.1s" }}
                  />
                  <div
                    className="w-1 h-2 bg-white/60 rounded-full animate-pulse"
                    style={{ animationDelay: "0.2s" }}
                  />
                </div>
              )}
            </div>
          )}

          {/* Speaking State */}
          {barState === "speaking" && (
            <div className="flex items-center justify-between w-full h-full">
              <div className="flex items-center gap-3 flex-1 min-w-0">
                <Volume2 className="h-4 w-4 text-purple-300 animate-pulse" />
                <span className="text-sm text-white/90 truncate">
                  {spokenText || "Playing response..."}
                </span>
              </div>
            </div>
          )}

          {/* Loading State */}
          {barState === "loading" && (
            <div className="w-full h-full flex flex-col items-center justify-center overflow-hidden px-2">
              <span className="text-xs text-white/70 truncate w-full text-center pb-1">
                {lastSubmittedValue}
              </span>
              <div className="loading-bar-thin"></div>
            </div>
          )}

          {/* Success State */}
          {barState === "success" && (
            <div className="flex items-center justify-between w-full h-full animate-success-fade">
              <div className="flex items-center gap-3 flex-1 min-w-0">
                <Check className="h-4 w-4 text-emerald-300" />
                <span className="text-sm font-medium text-emerald-100 truncate">
                  {lastSubmittedValue}
                </span>
              </div>
              <div className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-400">
                <Check size={12} className="text-emerald-900" />
              </div>
            </div>
          )}

          {/* Error State */}
          {barState === "error" && (
            <div className="flex items-center justify-between w-full h-full">
              <div className="flex items-center gap-3 flex-1 min-w-0">
                <AlertCircle className="h-4 w-4 text-red-300" />
                <span className="text-sm font-medium text-red-100 truncate">
                  {currentError || "Error occurred"}
                </span>
              </div>
              <div className="flex items-center justify-center h-6 w-6 rounded-full bg-red-400">
                <X size={12} className="text-red-900" />
              </div>
            </div>
          )}

          {/* Shrinking State */}
          {barState === "shrinking" && (
            <div className="opacity-0 w-full h-full transition-opacity duration-300" />
          )}
        </div>
      </div>
    </div>
  );
}
