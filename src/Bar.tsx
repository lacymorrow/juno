import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import { Check, Mic, Send, Volume2, X } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import tauriConfig from "../src-tauri/tauri.conf.json";
import { cn } from "./lib/utils";

// Get default window dimensions from tauri.conf.json
const floatingBarConfig = tauriConfig.app.windows.find(
  (window) => window.label === "floating-bar"
);
const DEFAULT_WIDTH = floatingBarConfig?.width || 110;
const DEFAULT_HEIGHT = floatingBarConfig?.height || 60;

// Constants for expanded size
const EXPANDED_WIDTH = 280;
const EXPANDED_HEIGHT = 70;

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

export function FloatingBar() {
  // Frontend state now mirrors backend state - no local state logic
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [transcriptionText, setTranscriptionText] = useState("");
  const [spokenText, setSpokenText] = useState("");
  const [_isAgentWorking, setIsAgentWorking] = useState(false);
  const [_isDictationMode, setIsDictationMode] = useState(false);
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Update window size based on bar state
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = await Window.getByLabel("floating-bar");
        switch (barState) {
          case "default":
            await appWindow?.setSize(
              new LogicalSize(DEFAULT_WIDTH, DEFAULT_HEIGHT)
            );
            break;
          case "shrinking":
          case "loading":
          case "finishing":
          case "expanding":
          case "input":
          case "success":
          case "listening":
          case "error":
          case "transcribing":
          case "speaking":
          case "dictating":
            await appWindow?.setSize(
              new LogicalSize(EXPANDED_WIDTH, EXPANDED_HEIGHT)
            );
            break;
        }
      } catch (err) {
        console.error("Failed to resize window:", err);
      }
    };
    resizeWindow();
  }, [barState]);

  // Listen for backend state updates
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>("bar-state-update", (event) => {
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

          // Handle input focus for input state
          if (data.barState === "input" && inputRef.current) {
            requestAnimationFrame(() => {
              if (inputRef.current) {
                inputRef.current.focus();
              }
            });
          }
        });
      } catch (error) {
        console.error("Failed to setup bar state listener:", error);
      }
    };

    setupListener();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Listen for window hover events
  useEffect(() => {
    let unlistenEnter: (() => void) | undefined;
    let unlistenLeave: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenEnter = await listen<null>("mouse-entered-window", () => {
          console.log("Mouse entered window bounds (event)");
          setIsWindowHovered(true);
        });
        unlistenLeave = await listen<null>("mouse-left-window", () => {
          console.log("Mouse left window bounds (event)");
          setIsWindowHovered(false);
        });
      } catch (error) {
        console.error("Failed to setup hover listeners:", error);
      }
    };

    setupListeners();
    return () => {
      if (unlistenEnter) unlistenEnter();
      if (unlistenLeave) unlistenLeave();
    };
  }, []);

  // Handle window focus changes
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

  // Handler functions that call backend commands
  const handleBarClick = async () => {
    try {
      await invoke("floating_bar_click");
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
      await invoke("floating_bar_input_change", { value });
    } catch (err) {
      console.error("Failed to handle input change:", err);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const query = inputValue.trim();
    if (!query) return;

    try {
      await invoke("floating_bar_submit", { query });
    } catch (err) {
      console.error("Failed to handle submit:", err);
    }
  };

  // Get bar styles based on state
  const getBarStyles = () => {
    switch (barState) {
      case "default":
        return "h-[20px] w-[60px] px-2";
      case "expanding":
        return "h-[40px] w-[240px] px-3";
      case "input":
        return "h-[40px] w-[240px] px-3";
      case "success":
        return "h-[40px] w-[240px] px-3";
      case "shrinking":
        return "h-[20px] w-[60px] px-2";
      case "loading":
        return "h-[40px] w-[240px] px-3";
      case "finishing":
        return "h-[20px] w-[60px] px-2";
      case "listening":
        return "h-[40px] w-[240px] px-3";
      case "error":
        return "h-[40px] w-[240px] px-3";
      case "transcribing":
        return "h-[40px] w-[240px] px-3";
      case "speaking":
        return "h-[40px] w-[240px] px-3";
      case "dictating":
        return "h-[40px] w-[240px] px-3";
      default:
        return "h-[20px] w-[60px] px-2";
    }
  };

  return (
    <div
      data-window-hovered={isWindowHovered}
      className="w-screen h-screen flex items-start justify-start bg-transparent"
    >
      <div className="relative z-50 p-3 bg-transparent">
        <div
          data-tauri-drag-region
          className={cn(
            `
            flex items-center justify-center bg-black/20 text-white
            rounded-full shadow-lg border border-white/10 overflow-hidden
            transition-all duration-300 ease-in-out
            [will-change:width,height,transform]
            [backface-visibility:hidden]
            [transform-origin:center]
            backdrop-blur-md
            ${getBarStyles()}
            ${barState === "default" ? "cursor-pointer" : ""}
            `,
            barState === "default" &&
              isWindowHovered &&
              "[transform:scale3d(1.05,1.05,1)]"
          )}
          onClick={barState === "default" ? handleBarClick : undefined}
        >
          {/* Default State Content */}
          {(barState === "default" || barState === "finishing") && (
            <div
              className={cn(
                "w-5 h-[4px] bg-emerald-400 rounded-full",
                "transition-all duration-300 ease-in-out",
                barState === "finishing" ? "opacity-0 animate-fade-in" : ""
              )}
            ></div>
          )}

          {/* Expanding/Input State Content */}
          {(barState === "expanding" || barState === "input") && (
            <form
              onSubmit={handleSubmit}
              className={`
                flex items-center justify-between w-full h-full
                transition-opacity duration-300 ease-in-out
                ${barState === "input" ? "opacity-100" : "opacity-0"}
              `}
            >
              <input
                ref={inputRef}
                type="text"
                value={inputValue}
                onChange={(e) => handleInputChange(e.target.value)}
                onBlur={handleInputBlur}
                placeholder="Type a command..."
                className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/50"
                disabled={barState !== "input"}
              />
              <button
                type="submit"
                className="text-muted-foreground hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
                disabled={barState !== "input"}
              >
                <Send size={12} />
              </button>
            </form>
          )}

          {/* Success State Content */}
          {barState === "success" && (
            <div className="flex items-center justify-between w-full h-full animate-success-fade">
              <span className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap pl-2 text-sm text-emerald-400 font-medium">
                {lastSubmittedValue}
              </span>
              <div className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-500">
                <Check size={12} className="text-black" />
              </div>
            </div>
          )}

          {/* Shrinking State - Empty to create clean transition */}
          {barState === "shrinking" && (
            <div className="opacity-0 w-full h-full"></div>
          )}

          {/* Loading State Content */}
          {barState === "loading" && (
            <div className="w-full h-full flex flex-col items-center justify-center overflow-hidden px-2">
              <span className="text-xs text-white/70 truncate w-full text-center pb-1">
                {lastSubmittedValue}
              </span>
              <div className="loading-bar-thin"></div>
            </div>
          )}

          {/* Listening State Content */}
          {barState === "listening" && (
            <div className="w-full h-full flex items-center justify-center overflow-hidden animate-pulse">
              <div className="mr-2 flex-shrink-0 relative">
                <Mic size={16} className="text-blue-400" />
                <div className="absolute -top-1 -right-1 w-2 h-2 bg-blue-400 rounded-full animate-pulse"></div>
              </div>
              <span className="text-sm text-blue-200 font-medium">
                Listening for voice...
              </span>
            </div>
          )}

          {/* Transcribing State Content */}
          {barState === "transcribing" && (
            <div className="w-full h-full flex items-center justify-start overflow-hidden px-3">
              <div className="mr-2 flex-shrink-0 relative">
                <Mic size={16} className="text-green-400" />
                <div className="absolute -top-1 -right-1 w-2 h-2 bg-green-400 rounded-full animate-bounce"></div>
              </div>
              <span className="text-sm text-green-200 truncate font-medium">
                {transcriptionText || "Transcribing..."}
              </span>
            </div>
          )}

          {/* Speaking State Content */}
          {barState === "speaking" && (
            <div className="w-full h-full flex items-center justify-start overflow-hidden px-3">
              <Volume2
                size={16}
                className="mr-2 text-purple-400 flex-shrink-0"
              />
              <span className="text-sm text-white/90 truncate">
                {spokenText || "Speaking..."}
              </span>
            </div>
          )}

          {/* Dictating State Content */}
          {barState === "dictating" && (
            <div className="w-full h-full flex items-center justify-start overflow-hidden px-3 animate-pulse">
              <div className="mr-2 flex-shrink-0 relative">
                <Mic size={16} className="text-orange-400" />
                <div className="absolute -top-1 -right-1 w-2 h-2 bg-orange-400 rounded-full animate-ping"></div>
              </div>
              <span className="text-sm text-orange-200 truncate font-medium">
                {transcriptionText || "Hold dictation key to dictate..."}
              </span>
            </div>
          )}

          {/* Error State Content */}
          {barState === "error" && (
            <div className="flex items-center justify-between w-full h-full">
              <span className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap pl-2 text-sm text-red-400 font-medium">
                {currentError || "Error processing"}
              </span>
              <div className="flex items-center justify-center h-6 w-6 rounded-full bg-red-500">
                <X size={12} className="text-black" />
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
