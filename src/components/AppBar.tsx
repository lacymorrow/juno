import { useEffect, useState, useRef, useCallback, FormEvent } from "react";
import { Mic, Zap, Volume2, MessageCircle, Keyboard, Send } from "lucide-react";
import { cn } from "@/lib/utils";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";

// === UI API IMPORTS ===
import {
  useUIElement,
  UIState,
  type UIStateData,
  type UIElementConfig,
} from "@/lib/ui-api";

// === COMPONENT DEFINITION ===

const getMainIcon = (uiState: UIState) => {
  switch (uiState) {
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
    case "dictation-ready":
      return <Keyboard size={14} className="text-orange-400" />;
    case "always-listening":
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
    case "dictation-ready":
      return "Ready for dictation mode";
    case "always-listening":
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
  if (!["listening", "transcribing", "always-listening"].includes(uiState)) {
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

  // === SIMPLIFIED STATE MANAGEMENT ===
  const [currentState, setCurrentState] = useState<UIStateData | null>(null);
  const [currentConfig, setCurrentConfig] = useState<UIElementConfig | null>(
    null
  );

  // === LOCAL STATE ===
  const [inputValue, setInputValue] = useState("");
  const [isWindowHovered] = useState(false);
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);

  // === SYNC UI API STATE ===
  useEffect(() => {
    if (state) {
      console.log("AppBar: Syncing UI API state:", state);
      setCurrentState(state);
      setInputValue(state.inputValue);
      setCurrentError(state.currentError);
      setAudioLevel(state.audioLevel || 0);
    }
  }, [state]);

  useEffect(() => {
    if (config) {
      console.log("AppBar: Syncing UI API config:", config);
      setCurrentConfig(config);
    }
  }, [config]);

  // === UPDATE UI API STATE ===
  useEffect(() => {
    if (manager && updateState) {
      updateState({
        inputValue,
        currentError,
        audioLevel,
      });
    }
  }, [updateState, manager, inputValue, currentError, audioLevel]);

  // === FOCUS MANAGEMENT ===
  useEffect(() => {
    if (currentState?.uiState === "input" && inputRef.current) {
      inputRef.current.focus();
    }
  }, [currentState?.uiState]);

  // Use UI API config with fallback defaults
  const uiConfig = currentConfig || {
    id: "floating-bar",
    type: "bar" as const,
    showVoiceIndicator: true,
    enableAnimations: true,
    autoHide: false,
    autoHideDelay: 3000,
    opacity: 0.95,
  };

  // === EVENT HANDLERS ===
  const handleBarClick = useCallback(async () => {
    if (click) {
      await click({ source: "bar" });
    }
  }, [click]);

  const handleInputChange = useCallback(
    (value: string) => {
      setInputValue(value);
      if (input) {
        input(value);
      }
    },
    [input]
  );

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      if (submit && inputValue.trim()) {
        await submit(inputValue.trim());
      }
    },
    [submit, inputValue]
  );

  const handleInputFocus = useCallback(async () => {
    if (focus) {
      await focus({ source: "input" });
    }
  }, [focus]);

  const handleInputBlur = useCallback(async () => {
    if (blur) {
      await blur({ source: "input" });
    }
  }, [blur]);

  // === STYLING ===
  const getContainerStyles = () => {
    let bgColor = "bg-black/90";

    switch (currentState?.voiceMode) {
      case "dictation":
        bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
        break;
      case "agent":
        bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
        break;
      default:
        if (currentState?.isDictationMode) {
          bgColor = "bg-gradient-to-r from-orange-600/98 to-orange-700/98";
        } else if (currentState?.isAgentWorking) {
          bgColor = "bg-gradient-to-r from-blue-600/98 to-blue-700/98";
        }
        break;
    }

    // Override for specific states
    if (currentState?.uiState === "error") {
      bgColor = "bg-gradient-to-r from-red-600/90 to-red-700/90";
    } else if (currentState?.uiState === "success") {
      bgColor = "bg-gradient-to-r from-emerald-600/90 to-emerald-700/90";
    } else if (currentState?.uiState === "always-listening") {
      bgColor = "bg-gradient-to-r from-blue-500/98 to-cyan-600/98";
    }

    const sizeStyles = ["default"].includes(currentState?.uiState || "default")
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const hoverEffect =
      currentState?.uiState === "default" && isWindowHovered
        ? "ring-2 ring-white/30"
        : "";

    const clickable = ["default", "dictation-ready"].includes(
      currentState?.uiState || "default"
    )
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
          ["default", "dictation-ready"].includes(
            currentState?.uiState || "default"
          )
            ? handleBarClick
            : undefined
        }
      >
        {/* Default State */}
        {(currentState?.uiState === "default" ||
          currentState?.uiState === "dictation-ready" ||
          currentState?.uiState === "finishing") && (
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon(currentState?.uiState || "default")}
            {uiConfig.showVoiceIndicator &&
              (currentState?.voiceMode !== "idle" ||
                currentState?.isDictationMode ||
                currentState?.isAgentWorking) && (
                <VoiceStatusIndicator variant="compact" className="ml-1" />
              )}
            {currentState?.isAlwaysListening && (
              <div
                className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
                data-tauri-drag-region
              />
            )}
          </div>
        )}

        {/* Input State */}
        {(currentState?.uiState === "expanding" ||
          currentState?.uiState === "input") && (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3",
              "transition-opacity duration-300 ease-in-out",
              currentState?.uiState === "input" ? "opacity-100" : "opacity-0"
            )}
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon(currentState?.uiState || "input")}
              <input
                ref={inputRef}
                type="text"
                value={inputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onFocus={handleInputFocus}
                onBlur={handleInputBlur}
                placeholder="Ask me anything..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                disabled={currentState?.uiState !== "input"}
              />
            </div>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={currentState?.uiState !== "input"}
            >
              <Send size={14} />
            </button>
          </form>
        )}

        {/* Active States */}
        {[
          "submitting",
          "loading",
          "speaking",
          "dictating",
          "transcribing",
          "agent-responding",
          "listening",
        ].includes(currentState?.uiState || "") && (
          <div
            className="flex items-center justify-between w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon(currentState?.uiState || "default")}
              <span
                className="text-sm font-medium truncate"
                data-tauri-drag-region
              >
                {getStatusText(
                  currentState?.uiState || "default",
                  currentError
                )}
              </span>
            </div>
            <AudioLevelIndicator
              uiState={currentState?.uiState || "default"}
              audioLevel={audioLevel}
            />
          </div>
        )}

        {/* Other states would go here similar to FloatingBar... */}
      </div>
    </div>
  );
}
