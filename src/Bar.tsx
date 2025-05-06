import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { PhysicalSize, Window } from "@tauri-apps/api/window";
import { Check, Send } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import tauriConfig from "../src-tauri/tauri.conf.json";
import { cn } from "./lib/utils";

// Get default window dimensions from tauri.conf.json
const floatingBarConfig = tauriConfig.app.windows.find(
  (window) => window.label === "floating-bar"
);
const DEFAULT_WIDTH = floatingBarConfig?.width || 110; // Fallback if not found
const DEFAULT_HEIGHT = floatingBarConfig?.height || 60; // Fallback if not found

// Constants for expanded size (consider adding to config later if needed)
const EXPANDED_WIDTH = 280;
const EXPANDED_HEIGHT = 70;

type BarState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "loading"
  | "finishing"
  | "success";

export function FloatingBar() {
  const [barState, setBarState] = useState<BarState>("loading");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const transitionTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const [isWindowHovered, setIsWindowHovered] = useState(false);

  // For debugging - log state changes
  useEffect(() => {
    console.log("Bar state changed to:", barState);
  }, [barState]);

  // Window resize effect
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        // Get the window by label to ensure we're targeting the correct window
        const appWindow = await Window.getByLabel("floating-bar");

        switch (barState) {
          case "default":
            // Smaller window size for collapsed bar (from tauri.conf.json)
            await appWindow?.setSize(
              new PhysicalSize(DEFAULT_WIDTH, DEFAULT_HEIGHT)
            );
            break;
          case "shrinking":
          case "loading":
          case "finishing":
          case "expanding":
          case "input":
          case "success":
            // Larger window size for expanded bar
            await appWindow?.setSize(
              new PhysicalSize(EXPANDED_WIDTH, EXPANDED_HEIGHT)
            );
            break;
        }
      } catch (err) {
        console.error("Failed to resize window:", err);
      }
    };

    resizeWindow();
  }, [barState]);

  const handleBarClick = () => {
    if (barState !== "default") return;

    // Start expansion animation
    setBarState("expanding");

    // After expansion animation completes, set to input state and focus the input
    transitionTimeoutRef.current = setTimeout(() => {
      setBarState("input");
      // Ensure input is focused after the state change is applied
      requestAnimationFrame(() => {
        if (inputRef.current) inputRef.current.focus();
      });
    }, 300); // Match the CSS transition duration
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const query = inputValue.trim();
    if (!query) return;

    // Store the submitted value to display during transitions
    setLastSubmittedValue(query);
    setInputValue(""); // Clear input immediately
    inputRef.current?.blur(); // Blur input

    // First show a brief success state
    setBarState("success");

    // After success animation, start shrinking animation
    transitionTimeoutRef.current = setTimeout(() => {
      setBarState("shrinking");

      // After shrinking animation, show loader and call backend
      transitionTimeoutRef.current = setTimeout(async () => {
        setBarState("loading");
        console.log("Calling backend with query:", query);

        try {
          // **** Start backend call ****
          // Invoke the command. We no longer need the direct result here.
          await invoke("submit_query", { query: query });
          console.log("Backend call invoked successfully");
          // **** Backend call finished ****

          // Transition to finishing after backend call *invocation* succeeds
          // The actual result processing happens in App.tsx via event listener
          setBarState("finishing");
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("input"); // Go back to input state
            // Ensure input is focused after state change
            requestAnimationFrame(() => {
              if (inputRef.current) inputRef.current.focus();
            });
          }, 300); // Finishing duration
        } catch (error) {
          console.error("Error submitting query:", error);
          // Handle error: go back to default after a brief finishing state
          setBarState("finishing"); // Use finishing state visually
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("input"); // Go back to input state even on error
            // Ensure input is focused after state change
            requestAnimationFrame(() => {
              if (inputRef.current) inputRef.current.focus();
            });
          }, 300);
        }
      }, 300); // Shrinking duration
    }, 600); // Success state duration
  };

  const handleInputBlur = () => {
    // Always shrink when input loses focus, regardless of content
    if (barState === "input") {
      // Start shrinking animation
      setBarState("shrinking");

      // After shrinking animation, return to default
      transitionTimeoutRef.current = setTimeout(() => {
        setBarState("default");
        setInputValue(""); // Clear input when shrinking back
      }, 300); // Match the CSS transition duration
    }
  };

  useEffect(() => {
    return () => {
      if (transitionTimeoutRef.current)
        clearTimeout(transitionTimeoutRef.current);
    };
  }, []);

  // Effect to listen for custom window hover events from backend
  useEffect(() => {
    let unlistenEnter: (() => void) | undefined;
    let unlistenLeave: (() => void) | undefined;

    const setupListeners = async () => {
      unlistenEnter = await listen<null>("mouse-entered-window", () => {
        console.log("Mouse entered window bounds (event)");
        setIsWindowHovered(true);
      });
      unlistenLeave = await listen<null>("mouse-left-window", () => {
        console.log("Mouse left window bounds (event)");
        setIsWindowHovered(false);
      });
    };

    setupListeners();

    return () => {
      unlistenEnter?.();
      unlistenLeave?.();
    };
  }, []); // Empty dependency array, runs once on mount

  // Effect to handle window focus changes
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      const currentWindow = Window.getCurrent();
      unlisten = await currentWindow.onFocusChanged(
        ({ payload: isFocused }) => {
          console.log("Window focus changed:", isFocused);
          if (isFocused) {
            // When window gains focus, if in default state, expand it
            if (barState === "default") {
              handleBarClick(); // Use existing click handler to expand and focus
            } else if (barState === "input" && inputRef.current) {
              // If already in input state, ensure focus
              inputRef.current.focus();
            }
          } else {
            // When window loses focus
            if (barState === "input" && !inputValue) {
              // If in input state and input is empty, collapse
              handleInputBlur(); // Use existing blur handler to shrink
            } else if (barState === "input" && inputRef.current) {
              // If it has text or is not in input state, just blur
              inputRef.current.blur();
            }
          }
        }
      );
    };

    setupListener();

    // Cleanup listener on component unmount or when dependencies change
    return () => {
      unlisten?.();
    };
    // Dependencies: Include states and handlers used inside the effect
  }, [barState, inputValue, handleBarClick, handleInputBlur]);

  // Determine dimensions based on state
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
        return "h-[20px] w-[60px] px-2";
      case "finishing":
        return "h-[20px] w-[60px] px-2";
      default:
        return "h-[20px] w-[60px] px-2";
    }
  };

  return (
    <div
      data-tauri-drag-region
      data-window-hovered={isWindowHovered}
      className={cn(
        "w-screen h-screen flex items-start justify-start p-1"
        // barState !== "input" && "cursor-pointer"
      )}
      onClick={(e) => {
        if (e.target === e.currentTarget && barState === "default") {
          // Allow clicking the background area to trigger expansion if needed
          // handleBarClick(); // Uncomment if clicking bg should expand
        }
      }}
    >
      {/* Container for the bar, positioned relative to the flex container */}
      <div className="relative z-50">
        {/* Universal Bar Container - Now positioned within the flex container */}
        <div
          className={cn(
            `
            flex items-center justify-center bg-black/90 backdrop-blur-md text-white
            rounded-full shadow-lg border border-white/20 overflow-hidden
            transition-all duration-300 ease-in-out
            ${getBarStyles()}
            ${barState === "default" ? "cursor-pointer" : ""}
            `,
            // Add slight size increase on hover only when in default state
            barState === "default" && isWindowHovered && "scale-105"
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
                onChange={(e) => setInputValue(e.target.value)}
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
                <Send size={12} className="" />
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
            <div className="w-full h-full flex items-center justify-center overflow-hidden">
              <div className="loading-bar-thin"></div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
