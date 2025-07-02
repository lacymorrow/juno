import { useEffect, useState, useCallback, FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
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
import { VoiceStatusIndicator } from "./VoiceStatusIndicator";
import tauriConfig from "../../src-tauri/tauri.conf.json";

// === TYPES ===
type UIState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "submitting"
  | "loading"
  | "finishing"
  | "success"
  | "listening"
  | "error"
  | "transcribing"
  | "speaking"
  | "dictating"
  | "always-listening"
  | "agent-responding"
  | "dictation-ready";

interface BarStateData {
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

// === CONSTANTS ===
const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 60,
  DEFAULT_HEIGHT: 20,
  EXPANDED_WIDTH: 280,
  EXPANDED_HEIGHT: 50,
};

// === COMPONENT DEFINITION ===

export function FloatingBar() {
  // === STATE MANAGEMENT ===
  const [barState, setBarState] = useState<BarStateData>({
    barState: "default",
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: "idle",
    agentState: null,
  });

  // === WINDOW DIMENSIONS ===
  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === "floating-bar"
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
  const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
  const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

  // === EVENT LISTENERS ===
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      unlisten = await listen<BarStateData>("bar-state-update", (event) => {
        console.log("FloatingBar: Received bar state update:", event.payload);
        setBarState(event.payload);
      });
    };

    setupListener().catch(console.error);

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // === WINDOW RESIZING ===
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const currentUiState = barState.barState as UIState;
        const isCompact = ["default", "dictation-ready"].includes(
          currentUiState
        );
        const currentWidth = isCompact ? defaultWidth : EXPANDED_WIDTH;
        const currentHeight = isCompact ? defaultHeight : EXPANDED_HEIGHT;

        console.log(
          `FloatingBar: Resizing window to ${currentWidth}x${currentHeight} for state: ${currentUiState}`
        );

        await appWindow.setSize(new LogicalSize(currentWidth, currentHeight));
      } catch (error) {
        console.error("Failed to resize floating bar window:", error);
      }
    };

    resizeWindow();
  }, [
    barState.barState,
    defaultWidth,
    defaultHeight,
    EXPANDED_WIDTH,
    EXPANDED_HEIGHT,
  ]);

  // === UI HANDLERS ===
  const handleClick = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          element_id: "floating-bar",
          interaction_type: "click",
          data: null,
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("Failed to handle bar click:", error);
    }
  }, []);

  const handleInputChange = useCallback(async (value: string) => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          element_id: "floating-bar",
          interaction_type: "input_change",
          data: { value },
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("Failed to handle input change:", error);
    }
  }, []);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const trimmedValue = barState.inputValue.trim();
      if (trimmedValue) {
        try {
          await invoke("ui_handle_interaction", {
            elementId: "floating-bar",
            interaction: {
              element_id: "floating-bar",
              interaction_type: "submit",
              data: { value: trimmedValue },
              timestamp: Date.now(),
            },
          });
        } catch (error) {
          console.error("Failed to handle bar submit:", error);
        }
      }
    },
    [barState.inputValue]
  );

  const handleFocus = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          element_id: "floating-bar",
          interaction_type: "focus",
          data: { is_focused: true },
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("Failed to handle focus:", error);
    }
  }, []);

  const handleBlur = useCallback(async () => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-bar",
        interaction: {
          element_id: "floating-bar",
          interaction_type: "blur",
          data: { is_focused: false },
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("Failed to handle blur:", error);
    }
  }, []);

  // === UI CALCULATIONS ===
  const currentUiState = barState.barState as UIState;
  const isCompact = ["default", "dictation-ready"].includes(currentUiState);
  const currentWidth = isCompact ? defaultWidth : EXPANDED_WIDTH;
  const currentHeight = isCompact ? defaultHeight : EXPANDED_HEIGHT;

  // === DYNAMIC STYLING ===
  const getContainerStyles = () => {
    const sizeStyles = isCompact
      ? "h-[20px] w-[60px] px-2"
      : "h-[50px] w-[280px] px-4";

    const clickable = ["default", "dictation-ready"].includes(currentUiState)
      ? "cursor-pointer"
      : "";

    return cn(
      "relative flex items-center justify-center",
      "text-white rounded-full shadow-lg border border-white/20",
      "transition-all duration-300 ease-in-out",
      "bg-black/90 backdrop-blur-md",
      sizeStyles,
      clickable
    );
  };

  // === HELPER FUNCTIONS ===
  const getMainIcon = () => {
    switch (currentUiState) {
      case "listening":
        return <Mic size={14} className="text-blue-400" />;
      case "transcribing":
        return <Mic size={14} className="animate-pulse text-blue-400" />;
      case "speaking":
        return <Volume2 size={14} className="text-green-400" />;
      case "loading":
      case "submitting":
        return <Loader2 size={14} className="animate-spin text-yellow-400" />;
      case "input":
      case "expanding":
        return <Sparkles size={14} className="text-white" />;
      case "dictation-ready":
        return <Type size={14} className="text-orange-400" />;
      case "always-listening":
        return <Mic size={14} className="text-blue-400 animate-pulse" />;
      case "error":
        return <AlertCircle size={14} className="text-red-400" />;
      case "success":
        return <Check size={14} className="text-green-400" />;
      default:
        return <Brain size={14} className="text-white" />;
    }
  };

  const AudioLevelIndicator = ({ audioLevel }: { audioLevel: number }) => {
    if (
      !["listening", "transcribing", "always-listening"].includes(
        currentUiState
      )
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

  return (
    <div className="w-screen h-screen relative overflow-hidden cursor-move">
      <div
        className={getContainerStyles()}
        style={{
          width: `${currentWidth}px`,
          height: `${currentHeight}px`,
        }}
        onClick={
          ["default", "dictation-ready"].includes(currentUiState)
            ? handleClick
            : undefined
        }
      >
        {/* Default/Compact States */}
        {isCompact && (
          <div className="flex items-center gap-2" data-tauri-drag-region>
            {getMainIcon()}
            {barState.voiceMode !== "idle" && (
              <VoiceStatusIndicator variant="compact" className="ml-1" />
            )}
          </div>
        )}

        {/* Active States with Audio Level */}
        {[
          "listening",
          "transcribing",
          "speaking",
          "dictating",
          "agent-responding",
        ].includes(currentUiState) && (
          <div
            className="flex items-center justify-between w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon()}
              <span
                className="text-sm font-medium truncate"
                data-tauri-drag-region
              >
                {currentUiState === "listening" && "Listening..."}
                {currentUiState === "transcribing" && "Converting speech..."}
                {currentUiState === "speaking" && "Playing response..."}
                {currentUiState === "dictating" && "Dictating text..."}
                {currentUiState === "agent-responding" && "Agent working..."}
              </span>
            </div>
            <AudioLevelIndicator audioLevel={barState.audioLevel} />
          </div>
        )}

        {/* Input State */}
        {(currentUiState === "expanding" || currentUiState === "input") && (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center justify-between w-full h-full gap-3",
              "transition-opacity duration-300 ease-in-out",
              currentUiState === "input" ? "opacity-100" : "opacity-0"
            )}
            data-tauri-drag-region
          >
            <div
              className="flex items-center gap-2 flex-1"
              data-tauri-drag-region
            >
              {getMainIcon()}
              <input
                type="text"
                value={barState.inputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onFocus={handleFocus}
                onBlur={handleBlur}
                placeholder="Ask me anything..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
                disabled={currentUiState !== "input"}
              />
            </div>
            <button
              type="submit"
              className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
              disabled={currentUiState !== "input"}
            >
              <Send size={14} />
            </button>
          </form>
        )}

        {/* Other states */}
        {["submitting", "loading", "error", "success", "shrinking"].includes(
          currentUiState
        ) && (
          <div
            className="flex items-center justify-center w-full h-full"
            data-tauri-drag-region
          >
            <div className="flex items-center gap-2" data-tauri-drag-region>
              {getMainIcon()}
              <span className="text-sm font-medium" data-tauri-drag-region>
                {currentUiState === "submitting" && "Sending..."}
                {currentUiState === "loading" && "Processing..."}
                {currentUiState === "error" &&
                  (barState.currentError || "Error occurred")}
                {currentUiState === "success" && "Complete!"}
                {currentUiState === "shrinking" && ""}
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
