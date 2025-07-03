import React, {
  useEffect,
  useState,
  useRef,
  useCallback,
  FormEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic, Zap, Volume2, MessageCircle, Keyboard, Send } from "lucide-react";
import { cn } from "@/lib/utils";
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import { UI } from "@/lib/constants.generated";

// === TYPES ===

interface UIStateData {
  barState: string;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  audioLevel: number;
  voiceMode: string;
  agentState: string | null;
}

interface FloatingBarConfig {
  show_voice_indicator: boolean;
  enable_animations: boolean;
  auto_hide: boolean;
  auto_hide_delay: number;
  opacity: number;
}

// === ICON AND TEXT HELPERS ===

const getMainIcon = (uiState: string) => {
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

const getStatusText = (uiState: string, currentError: string | null) => {
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
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "Agent is responding...";
    default:
      return "Ready";
  }
};

// === AUDIO LEVEL INDICATOR ===

const AudioLevelIndicator = ({
  uiState,
  audioLevel,
}: {
  uiState: string;
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

// === MAIN COMPONENT ===

const FloatingBar: React.FC = () => {
  // === STATE MANAGEMENT ===
  const [stateData, setStateData] = useState<UIStateData | null>(null);
  const [config, setConfig] = useState<FloatingBarConfig | null>(null);
  const [inputValue, setInputValue] = useState("");
  const [isVisible] = useState(true);

  const inputRef = useRef<HTMLInputElement>(null);

  // === BACKEND EVENT LISTENERS ===
  useEffect(() => {
    const unlisten = listen<UIStateData>("bar-state-update", (event) => {
      console.log("FloatingBar: Received bar-state-update:", event.payload);
      setStateData(event.payload);
      setInputValue(event.payload.inputValue || "");
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Listen for config changes
  useEffect(() => {
    const unlisten = listen<FloatingBarConfig>(
      "floating-bar-config-changed",
      (event) => {
        console.log("FloatingBar: Received config update:", event.payload);
        setConfig(event.payload);
      }
    );

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Load initial config
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const initialConfig = await invoke<FloatingBarConfig>(
          "ui_get_bar_config"
        );
        setConfig(initialConfig);
      } catch (error) {
        console.warn("Failed to load initial bar config:", error);
        // Use defaults
        setConfig({
          show_voice_indicator: true,
          enable_animations: true,
          auto_hide: false,
          auto_hide_delay: 3000,
          opacity: 0.95,
        });
      }
    };

    loadConfig();
  }, []);

  // === FOCUS MANAGEMENT ===
  useEffect(() => {
    if (stateData?.barState === UI.BAR_STATES_INPUT && inputRef.current) {
      inputRef.current.focus();
    }
  }, [stateData?.barState]);

  // === EVENT HANDLERS ===
  const handleBarClick = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          interaction_type: "click",
          data: { source: "bar" },
        },
      });
    } catch (error) {
      console.error("Failed to handle bar click:", error);
    }
  }, []);

  const handleInputChange = useCallback((value: string) => {
    setInputValue(value);
  }, []);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      if (inputValue.trim()) {
        try {
          await invoke("ui_handle_interaction", {
            elementId: "floating-bar",
            interaction: {
              interaction_type: "submit",
              data: { query: inputValue.trim() },
            },
          });
        } catch (error) {
          console.error("Failed to submit query:", error);
        }
      }
    },
    [inputValue]
  );

  const handleInputFocus = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          interaction_type: "focus",
          data: { source: "input" },
        },
      });
    } catch (error) {
      console.error("Failed to handle input focus:", error);
    }
  }, []);

  const handleInputBlur = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          interaction_type: "blur",
          data: { source: "input" },
        },
      });
    } catch (error) {
      console.error("Failed to handle input blur:", error);
    }
  }, []);

  // === STYLING ===
  const getContainerStyles = () => {
    const currentState = stateData?.barState || UI.BAR_STATES_DEFAULT;
    let bgColor = "bg-black/90";

    // Voice mode specific styling
    switch (stateData?.voiceMode) {
      case UI.VOICE_MODES_DICTATION:
        bgColor = "bg-gradient-to-r from-orange-600/90 to-orange-700/90";
        break;
      case UI.VOICE_MODES_AGENT:
        bgColor = "bg-gradient-to-r from-blue-600/90 to-blue-700/90";
        break;
      default:
        if (stateData?.isDictationMode) {
          bgColor = "bg-gradient-to-r from-orange-600/98 to-orange-700/98";
        } else if (stateData?.isAgentWorking) {
          bgColor = "bg-gradient-to-r from-blue-600/98 to-blue-700/98";
        }
        break;
    }

    // State specific overrides
    if (currentState === UI.BAR_STATES_ERROR) {
      bgColor = "bg-gradient-to-r from-red-600/90 to-red-700/90";
    } else if (currentState === UI.BAR_STATES_SUCCESS) {
      bgColor = "bg-gradient-to-r from-emerald-600/90 to-emerald-700/90";
    } else if (currentState === UI.BAR_STATES_ALWAYS_LISTENING) {
      bgColor = "bg-gradient-to-r from-blue-500/98 to-cyan-600/98";
    }

    // Size based on state
    const sizeStyles =
      currentState === UI.BAR_STATES_DEFAULT
        ? "h-[20px] w-[60px] px-2"
        : "h-[50px] w-[280px] px-4";

    const clickable = [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_DICTATION_READY,
    ].includes(currentState as any)
      ? "cursor-pointer"
      : "";

    return cn(
      bgColor,
      sizeStyles,
      clickable,
      "rounded-full backdrop-blur-xl border border-white/20 transition-all duration-300 ease-out shadow-lg",
      "fixed top-4 left-1/2 transform -translate-x-1/2 z-50"
    );
  };

  // === RENDER ===
  if (!isVisible) {
    return null;
  }

  const currentState = stateData?.barState || UI.BAR_STATES_DEFAULT;
  const currentConfig = config || {
    show_voice_indicator: true,
    enable_animations: true,
    auto_hide: false,
    auto_hide_delay: 3000,
    opacity: 0.95,
  };

  return (
    <div
      className={getContainerStyles()}
      style={{ opacity: currentConfig.opacity }}
      onClick={
        [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
          currentState as any
        )
          ? handleBarClick
          : undefined
      }
    >
      {/* Default State */}
      {(currentState === UI.BAR_STATES_DEFAULT ||
        currentState === UI.BAR_STATES_DICTATION_READY ||
        currentState === UI.BAR_STATES_FINISHING) && (
        <div className="flex items-center gap-2" data-tauri-drag-region>
          {getMainIcon(currentState)}
          {currentConfig.show_voice_indicator &&
            (stateData?.voiceMode !== UI.VOICE_MODES_IDLE ||
              stateData?.isDictationMode ||
              stateData?.isAgentWorking) && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          {stateData?.isAlwaysListening && (
            <div
              className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
              data-tauri-drag-region
            />
          )}
        </div>
      )}

      {/* Input State */}
      {(currentState === UI.BAR_STATES_EXPANDING ||
        currentState === UI.BAR_STATES_INPUT) && (
        <form
          onSubmit={handleSubmit}
          className={cn(
            "flex items-center justify-between w-full h-full gap-3",
            "transition-opacity duration-300 ease-in-out",
            currentState === UI.BAR_STATES_INPUT ? "opacity-100" : "opacity-0"
          )}
          data-tauri-drag-region
        >
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon(currentState)}
            <input
              ref={inputRef}
              type="text"
              value={inputValue}
              onChange={(e) => handleInputChange(e.target.value)}
              onFocus={handleInputFocus}
              onBlur={handleInputBlur}
              placeholder="Ask me anything..."
              className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
              disabled={currentState !== UI.BAR_STATES_INPUT}
            />
          </div>
          <button
            type="submit"
            className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
            disabled={currentState !== UI.BAR_STATES_INPUT}
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
      ].includes(currentState as any) && (
        <div
          className="flex items-center justify-between w-full h-full"
          data-tauri-drag-region
        >
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon(currentState)}
            <span
              className="text-sm font-medium truncate"
              data-tauri-drag-region
            >
              {getStatusText(currentState, stateData?.currentError || null)}
            </span>
          </div>
          <AudioLevelIndicator
            uiState={currentState}
            audioLevel={stateData?.audioLevel || 0}
          />
        </div>
      )}

      {/* Success/Error States */}
      {(currentState === UI.BAR_STATES_SUCCESS ||
        currentState === UI.BAR_STATES_ERROR) && (
        <div
          className="flex items-center gap-2 w-full h-full"
          data-tauri-drag-region
        >
          {getMainIcon(currentState)}
          <span className="text-sm font-medium truncate" data-tauri-drag-region>
            {getStatusText(currentState, stateData?.currentError || null)}
          </span>
        </div>
      )}
    </div>
  );
};

export default FloatingBar;
