import { cn } from "@/lib/utils";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import {
  AlertCircle,
  Brain,
  Check,
  Loader2,
  Mic,
  MicOff,
  Send,
  Sparkles,
  Type,
  Volume2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import { useInvoke } from "@/hooks/useInvoke";
import { useEventListener } from "@/hooks/useEventListener";
import { useWindowSize } from "@/hooks/useWindowSize";
import type { BarState, BarStateData, FloatingBarConfig, WindowConfig } from "@/types/floating-bar";
import { FLOATING_BAR_DIMENSIONS } from "@/types/floating-bar";

// Get default window dimensions from tauri.conf.json
const floatingBarConfig = tauriConfig.app.windows.find(
  (window: WindowConfig) => window.label === "floating-bar"
);
const DEFAULT_WIDTH = floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
const DEFAULT_HEIGHT = floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

// Types are now imported from shared types

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

  const { invokeCommand } = useInvoke();
  const { resizeWindow } = useWindowSize("floating-bar");

  // UI state
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const [isAnimatingSize, setIsAnimatingSize] = useState(false);
  // @ts-ignore - Currently commented out in display logic but may be re-enabled in future
  const [showTooltip, setShowTooltip] = useState(false);
  const [config, setConfig] = useState<FloatingBarConfig>({
    showVoiceIndicator: true,
    enableAnimations: true,
    autoHide: false,
    autoHideDelay: 3000,
    opacity: 0.95,
  });

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

  // Update window size based on bar state
  useEffect(() => {
    const isCompact = ["default", "finishing"].includes(barState);
    
    const targetSize = isCompact
      ? { width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT }
      : { width: EXPANDED_WIDTH, height: EXPANDED_HEIGHT };
    
    resizeWindow(targetSize);
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

  const handleInputChange = useCallback(async (value: string) => {
    setInputValue(value);
    await invokeCommand("floating_bar_input_change", { value });
  }, [invokeCommand]);

  const handleSubmit = useCallback(async (e: FormEvent) => {
    e.preventDefault();
    const query = inputValue.trim();
    if (!query) return;

    await invokeCommand("floating_bar_submit", { query });
  }, [inputValue, invokeCommand]);

  // Get main icon based on enhanced state
  const getMainIcon = () => {
    switch (barState) {
      case "default":
        return <Sparkles className="h-4 w-4 text-emerald-400" />;
      case "dictation_ready":
        return <MicOff className="h-4 w-4 text-muted-foreground" />;
      case "dictation_active":
      case "dictating":
        return <Type className="h-4 w-4 text-orange-500" />;
      case "dictation_processing":
      case "transcribing":
        return <Loader2 className="h-4 w-4 text-orange-500 animate-spin" />;
      case "agent_listening":
      case "listening":
        return <Brain className="h-4 w-4 text-blue-500" />;
      case "agent_thinking":
        return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
      case "agent_responding":
        return <Brain className="h-4 w-4 text-blue-500 animate-pulse" />;
      case "always-listening":
        return <Mic className="h-4 w-4 text-blue-400" />;
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

  // Get enhanced status text for tooltip
  const getStatusText = () => {
    switch (barState) {
      case "default":
        if (isAlwaysListening) return "Always listening for wake words";
        if (isDictationMode) return "Dictation mode active";
        if (isAgentWorking) return "Agent working...";
        return "Click to interact • Alt+D for AI • Option+Space to dictate";
      case "dictation_ready":
        return "Hold Option+Space to start dictating";
      case "dictation_active":
      case "dictating":
        return "Dictating... Release key to finish";
      case "dictation_processing":
      case "transcribing":
        return "Processing dictation...";
      case "agent_listening":
      case "listening":
        return "Listening for voice command...";
      case "agent_thinking":
        return "AI is thinking...";
      case "agent_responding":
        return "AI is responding...";
      case "speaking":
        return "Playing AI response";
      case "loading":
        return "Processing request...";
      case "success":
        return "Task completed successfully";
      case "error":
        return currentError || "An error occurred";
      case "always-listening":
        return "Always listening for wake words";
      default:
        return "Voice assistant ready";
    }
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

    const sizeStyles = [
      "default",
      "dictation_ready",
      "shrinking",
      "finishing",
    ].includes(barState)
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

  // Audio level visualization component
  const AudioLevelIndicator = () => {
    if (
      ![
        "dictation_active",
        "dictating",
        "agent_listening",
        "listening",
      ].includes(barState)
    )
      return null;

    return (
      <div
        className="flex items-center gap-1 ml-2 cursor-move"
        data-tauri-drag-region
      >
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            data-tauri-drag-region
            className={cn(
              "w-1 rounded-full transition-all duration-150",
              audioLevel > (i + 1) * 20 ? "bg-white h-3" : "bg-white/30 h-1"
            )}
          />
        ))}
      </div>
    );
  };

  return (
    <div
      data-tauri-drag-region
      className="w-screen h-screen flex items-start justify-start relative"
    >
      {/* Tooltip */}
      {/* {showTooltip && barState === "default" && (
        <div
          className="absolute top-16 left-8 z-50 animate-fade-in pointer-events-none cursor-move"
          data-tauri-drag-region
        >
          <div
            className="bg-black/90 text-white text-xs px-3 py-2 rounded-lg border border-white/20 backdrop-blur-md max-w-xs cursor-move"
            data-tauri-drag-region
          >
            {getStatusText()}
          </div>
        </div>
      )} */}

      <div className="relative z-50 p-3 bg-transparent" data-tauri-drag-region>
        <div
          data-tauri-drag-region
          className={getContainerStyles()}
          style={{ opacity: config.opacity }}
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
              {getMainIcon()}
              {config.showVoiceIndicator &&
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
            "dictation_active",
            "dictation_processing",
            "dictating",
            "transcribing",
            "agent_listening",
            "agent_thinking",
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
                {getMainIcon()}
                <div className="flex-1 min-w-0" data-tauri-drag-region>
                  <div
                    className="text-sm font-medium truncate"
                    data-tauri-drag-region
                  >
                    {getStatusText()}
                  </div>
                  {transcriptionText && (
                    <div
                      className="text-xs text-white/70 truncate"
                      data-tauri-drag-region
                    >
                      "{transcriptionText}"
                    </div>
                  )}
                </div>
              </div>
              <AudioLevelIndicator />
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
                  {spokenText || "Playing response..."}
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
                <Loader2 className="h-4 w-4 animate-spin" />
                <span className="text-sm font-medium" data-tauri-drag-region>
                  Processing
                </span>
              </div>
              {lastSubmittedValue && (
                <div
                  className="text-xs text-white/70 truncate w-full text-center"
                  data-tauri-drag-region
                >
                  {lastSubmittedValue}
                </div>
              )}
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
                <Check className="h-4 w-4 text-emerald-300" />
                <span
                  className="text-sm font-medium text-emerald-100 truncate"
                  data-tauri-drag-region
                >
                  {lastSubmittedValue}
                </span>
              </div>
              <div
                className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-400"
                data-tauri-drag-region
              >
                <Check size={12} className="text-emerald-900" />
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
            />
          )}
        </div>
      </div>
    </div>
  );
}
