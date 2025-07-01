import { cn } from "@/lib/utils";
import { Window } from "@tauri-apps/api/window";
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
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import { useWindowSize } from "@/hooks/useWindowSize";
import type {
  BarState,
  FloatingBarConfig,
  WindowConfig,
} from "@/types/floating-bar";
import { FLOATING_BAR_DIMENSIONS } from "@/types/floating-bar";

// === NEW UI API IMPORTS ===
import { useUIElement, UIState, type UIElementConfig } from "@/lib/ui-api";

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

// Convert UI API state to legacy BarState for backward compatibility
const convertUIStateToBarState = (uiState: UIState): BarState => {
  switch (uiState) {
    case "default":
      return "default";
    case "expanding":
      return "expanding";
    case "input":
      return "input";
    case "shrinking":
      return "shrinking";
    case "submitting":
      return "submitting";
    case "loading":
      return "loading";
    case "finishing":
      return "finishing";
    case "success":
      return "success";
    case "listening":
      return "listening";
    case "error":
      return "error";
    case "transcribing":
      return "transcribing";
    case "speaking":
      return "speaking";
    case "dictating":
      return "dictating";
    case "always-listening":
      return "always-listening";
    case "agent-responding":
      return "agent_responding";
    case "dictation-ready":
      return "dictation_ready";
    default:
      return "default";
  }
};

// Convert UI API config to legacy FloatingBarConfig
const convertUIConfigToBarConfig = (
  uiConfig: UIElementConfig
): FloatingBarConfig => ({
  showVoiceIndicator: uiConfig.showVoiceIndicator,
  enableAnimations: uiConfig.enableAnimations,
  autoHide: uiConfig.autoHide,
  autoHideDelay: uiConfig.autoHideDelay,
  opacity: uiConfig.opacity,
});

// Get main icon based on enhanced state
const getMainIcon = (barState: BarState) => {
  switch (barState) {
    case "default":
      return <Sparkles className="h-4 w-4 text-emerald-400" />;
    case "dictation_ready":
      return <MicOff className="h-4 w-4 text-muted-foreground" />;
    case "dictating":
      return <Type className="h-4 w-4 text-orange-500" />;
    case "transcribing":
      return <Loader2 className="h-4 w-4 text-orange-500 animate-spin" />;
    case "listening":
      return <Brain className="h-4 w-4 text-blue-500" />;
    case "submitting":
      return <Loader2 className="h-4 w-4 text-blue-400 animate-spin" />;
    case "loading":
      return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
    case "agent_responding":
      return <Brain className="h-4 w-4 text-blue-500 animate-pulse" />;
    case "always-listening":
      return <Mic className="h-4 w-4 text-blue-400" />;
    case "speaking":
      return <Volume2 className="h-4 w-4 text-purple-500" />;
    case "success":
      return <Check className="h-4 w-4 text-emerald-500" />;
    case "error":
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    default:
      return <Mic className="h-4 w-4 text-blue-500" />;
  }
};

// Get enhanced status text for tooltip
const getStatusText = (
  barState: BarState,
  currentError: string | null,
  agentState?: string | null
) => {
  switch (barState) {
    case "dictation_ready":
      return "Hold Option+Space to start dictating";
    case "dictating":
      return "Dictating... Release key to finish";
    case "transcribing":
      return "Processing dictation...";
    case "listening":
      return "Listening for voice command...";
    case "submitting":
      return "Submitting query...";
    case "loading":
      return "AI is processing...";
    case "agent_responding":
      return "AI is responding...";
    case "speaking":
      return "Playing AI response";
    case "success":
      // Check agent state to determine if it was actually successful
      if (agentState === "Failed") {
        return "Task failed";
      } else if (agentState === "Cancelled") {
        return "Task cancelled";
      } else if (agentState === "Offline") {
        return "Connection unavailable";
      } else {
        return "Task completed successfully";
      }
    case "error":
      return currentError || "An error occurred";
    case "always-listening":
      return "Always listening for wake words";
    default:
      return "Voice assistant ready";
  }
};

// Audio level visualization component
const AudioLevelIndicator = ({
  barState,
  audioLevel,
}: {
  barState: BarState;
  audioLevel: number;
}) => {
  if (!["dictating", "listening"].includes(barState)) return null;

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

export function FloatingBar() {
  // === NEW UI API INTEGRATION ===
  const { state, config, click, input, submit, blur, focus } = useUIElement(
    "floating-bar",
    "bar"
  );

  // Enhanced state management - now using UI API state
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

  const { resizeWindow } = useWindowSize("floating-bar");

  // UI state
  const [isAnimatingSize, setIsAnimatingSize] = useState(false);
  // @ts-ignore - Currently commented out in display logic but may be re-enabled in future
  const [showTooltip, setShowTooltip] = useState(false);
  const [barConfig, setBarConfig] = useState<FloatingBarConfig>({
    showVoiceIndicator: true,
    enableAnimations: true,
    autoHide: false,
    autoHideDelay: 3000,
    opacity: 0.95,
  });

  const inputRef = useRef<HTMLInputElement>(null);
  const tooltipTimeoutRef = useRef<NodeJS.Timeout>();

  // === SYNC UI API STATE WITH LOCAL STATE ===
  useEffect(() => {
    if (state) {
      console.log("FloatingBar: Syncing UI API state:", state);
      setBarState(convertUIStateToBarState(state.uiState));
      setInputValue(state.inputValue);
      setLastSubmittedValue(state.lastSubmittedValue);
      setCurrentError(state.currentError);
      setTranscriptionText(state.transcriptionText);
      setSpokenText(state.spokenText);
      setIsAgentWorking(state.isAgentWorking);
      setIsDictationMode(state.isDictationMode);
      setIsAlwaysListening(state.isAlwaysListening);
      setAudioLevel(state.audioLevel || 0);
      setVoiceMode(
        state.voiceMode === "dictation"
          ? "dictation"
          : state.voiceMode === "agent"
          ? "agent"
          : "idle"
      );
      setAgentState(state.agentState === "idle" ? null : state.agentState);

      // Auto-focus input when in input state
      if (state.uiState === "input" && inputRef.current) {
        requestAnimationFrame(() => {
          inputRef.current?.focus();
        });
      }
    }
  }, [state]);

  // === SYNC UI API CONFIG WITH LOCAL CONFIG ===
  useEffect(() => {
    if (config) {
      console.log("FloatingBar: Syncing UI API config:", config);
      setBarConfig(convertUIConfigToBarConfig(config));
    }
  }, [config]);

  // Load initial state and config
  useEffect(() => {
    // Initial state and config are loaded automatically by useUIElement hook
  }, []);

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
    if (barConfig.enableAnimations) {
      setIsAnimatingSize(["expanding", "shrinking"].includes(barState));
    }
  }, [barState, barConfig.enableAnimations]);

  // Cleanup tooltip timeout on unmount
  useEffect(() => {
    return () => {
      if (tooltipTimeoutRef.current) {
        clearTimeout(tooltipTimeoutRef.current);
      }
    };
  }, []);

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
            // Use UI API instead of direct command
            if (focus) {
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
  }, [barState, focus]);

  // === UPDATED HANDLER FUNCTIONS USING UI API ===
  const handleBarClick = useCallback(async () => {
    if (click) {
      await click();
    }
  }, [click]);

  const handleInputBlur = useCallback(async () => {
    if (blur) {
      await blur();
    }
  }, [blur]);

  const handleInputChange = useCallback(
    async (value: string) => {
      setInputValue(value);
      if (input) {
        await input(value);
      }
    },
    [input]
  );

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const query = inputValue.trim();
      if (!query) return;

      if (submit) {
        await submit(query);
      }
    },
    [inputValue, submit]
  );

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

    const hoverEffect = "";

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

  return (
    <div
      data-tauri-drag-region
      className="w-screen h-screen flex items-start justify-start relative overflow-hidden"
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
            {getStatusText(barState, currentError, agentState)}
          </div>
        </div>
      )} */}

      <div className="relative z-50 p-3 bg-transparent" data-tauri-drag-region>
        <div
          data-tauri-drag-region
          className={getContainerStyles()}
          style={{ opacity: barConfig.opacity }}
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
              {barConfig.showVoiceIndicator &&
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
                    {getStatusText(barState, currentError, agentState)}
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
                  {spokenText || "Playing response..."}
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
                <Loader2 className="h-4 w-4 animate-spin text-blue-400" />
                <span className="text-sm font-medium" data-tauri-drag-region>
                  Submitting
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
                  {getStatusText(barState, currentError, agentState)}
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
            />
          )}
        </div>
      </div>
    </div>
  );
}
