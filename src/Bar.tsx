import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import { Check, Mic, Send, Volume2, X } from "lucide-react";
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
  | "success"
  | "listening"
  | "error"
  | "transcribing"
  | "speaking";

export function FloatingBar() {
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [transcriptionText, setTranscriptionText] = useState<string>("");
  const [spokenText, setSpokenText] = useState<string>("");
  const inputRef = useRef<HTMLInputElement>(null);
  const transitionTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const isPreparingToDrag = useRef(false); // Added: Flag for drag operation

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
            // Larger window size for expanded bar
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

    // Emit events to trigger the standardized loading sequence.
    // The listeners in this component will handle the UI changes.
    try {
      // Announce that a query is about to be submitted.
      await emit("will-submit-query", { query });
      console.log(
        "FloatingBar: Emitted will-submit-query, invoking backend for:",
        query
      );

      // **** Start backend call ****
      await invoke("submit_query", { query: query });
      console.log(
        "FloatingBar: Backend call invoked successfully, emitting did-submit-query (success)"
      );
      // **** Backend call finished ****
      await emit("did-submit-query", { success: true });
    } catch (error) {
      console.error("FloatingBar: Error submitting query:", error);
      console.log("FloatingBar: Emitting did-submit-query (failure)");
      await emit("did-submit-query", { success: false });
    }
  };

  const handleInputBlur = () => {
    // Only shrink if the bar is in 'input' state AND the input field is empty (trimmed).
    // AND a drag operation isn't being initiated on the bar itself.
    if (isPreparingToDrag.current) {
      console.log(
        "handleInputBlur: Potential drag operation in progress, not shrinking."
      );
      return;
    }

    if (barState === "input" && !inputValue.trim()) {
      console.log(
        "handleInputBlur: Shrinking bar because state is 'input' and inputValue is empty."
      );
      // Start shrinking animation
      setBarState("shrinking");

      // Clear any existing timeout that might conflict before setting a new one.
      if (transitionTimeoutRef.current) {
        clearTimeout(transitionTimeoutRef.current);
      }
      // After shrinking animation, return to default
      transitionTimeoutRef.current = setTimeout(() => {
        setInputValue(""); // Clear input when shrinking back to default
        setBarState("default");
      }, 300); // Match the CSS transition duration
    } else if (barState === "input") {
      console.log(
        "handleInputBlur: Input blurred in 'input' state but inputValue is not empty. Bar remains expanded."
      );
    }
  };

  useEffect(() => {
    return () => {
      if (transitionTimeoutRef.current)
        clearTimeout(transitionTimeoutRef.current);
    };
  }, []);

  // Effect to listen for query submission lifecycle events
  useEffect(() => {
    let unlistenWillSubmit: (() => void) | undefined;
    let unlistenDidSubmit: (() => void) | undefined;

    const setupSubmitListeners = async () => {
      unlistenWillSubmit = await listen<{ query: string }>(
        "will-submit-query",
        (event) => {
          console.log("FloatingBar Event: will-submit-query", event.payload);
          const { query } = event.payload;
          setLastSubmittedValue(query);
          setInputValue(""); // Clear input field
          setCurrentError(null); // Clear any previous error

          if (transitionTimeoutRef.current)
            clearTimeout(transitionTimeoutRef.current);
          setBarState("success");
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("shrinking");
            transitionTimeoutRef.current = setTimeout(() => {
              setBarState("loading");
            }, 300); // Shrinking duration
          }, 600); // Success state duration
        }
      );

      unlistenDidSubmit = await listen<{ success: boolean; error?: string }>(
        "did-submit-query",
        (event) => {
          console.log(
            "FloatingBar Event: did-submit-query",
            event.payload,
            "current state:",
            barState
          );
          if (barState === "loading") {
            if (transitionTimeoutRef.current)
              clearTimeout(transitionTimeoutRef.current);

            if (event.payload.success) {
              setBarState("finishing");
              transitionTimeoutRef.current = setTimeout(() => {
                setBarState("input");
                requestAnimationFrame(() => {
                  if (inputRef.current) inputRef.current.focus();
                });
              }, 300); // Finishing duration
            } else {
              // Error handling
              setCurrentError(
                event.payload.error ||
                  `Failed: ${lastSubmittedValue}` ||
                  "An unexpected error occurred."
              );
              setBarState("error");
              transitionTimeoutRef.current = setTimeout(() => {
                setBarState("input");
                setCurrentError(null); // Clear error after timeout
                requestAnimationFrame(() => {
                  if (inputRef.current) inputRef.current.focus();
                });
              }, 3000); // Error state duration
            }
          }
        }
      );
    };

    setupSubmitListeners();
    return () => {
      unlistenWillSubmit?.();
      unlistenDidSubmit?.();
    };
  }, [barState, inputRef]); // barState needed for the conditional in unlistenDidSubmit

  // Effect to listen for dictation lifecycle events
  useEffect(() => {
    let unlistenDictationStarted: (() => void) | undefined;
    let unlistenDictationFinished: (() => void) | undefined;
    let unlistenDictationPartialResult: (() => void) | undefined; // Listener for partial results

    const setupDictationListeners = async () => {
      unlistenDictationStarted = await listen<null>(
        "app-dictation-started",
        () => {
          console.log("FloatingBar Event: app-dictation-started");
          if (transitionTimeoutRef.current)
            clearTimeout(transitionTimeoutRef.current);
          setInputValue(""); // Clear any input text
          setTranscriptionText(""); // Clear previous transcription
          setBarState("listening");
        }
      );

      // Listen for partial dictation results
      unlistenDictationPartialResult = await listen<{ partial: string }>(
        "app-dictation-partial-result",
        (event) => {
          console.log(
            "FloatingBar Event: app-dictation-partial-result",
            event.payload
          );
          if (barState === "listening" || barState === "transcribing") {
            setTranscriptionText(event.payload.partial);
            setBarState("transcribing"); // Ensure state is transcribing when we have partials
          }
        }
      );

      unlistenDictationFinished = await listen<{
        query: string | null;
        error?: string;
      }>("app-dictation-finished", (event) => {
        console.log("FloatingBar Event: app-dictation-finished", event.payload);
        setTranscriptionText(""); // Clear transcription on finish

        if (barState === "listening" || barState === "transcribing") {
          // Only act if we were in a dictation state
          if (transitionTimeoutRef.current)
            clearTimeout(transitionTimeoutRef.current);
          if (event.payload.query) {
            // A query was successfully dictated.
            // Transition to 'input' state. The dictation handler (elsewhere)
            // should then emit 'will-submit-query' which will trigger the processing flow.
            setBarState("input");
            // The actual inputValue will be set by the global dictation handler that emits will-submit-query
            // For now, we can pre-fill it, but the will-submit-query will overwrite it if dictation handler sets it differently.
            // setInputValue(event.payload.query); // Optional: pre-fill input for immediate visibility
            requestAnimationFrame(() => {
              if (inputRef.current) inputRef.current.focus();
            });
          } else {
            // No query from dictation (e.g., cancelled, error). Revert to default.
            setBarState("shrinking");
            transitionTimeoutRef.current = setTimeout(() => {
              setBarState("default");
            }, 300); // Shrinking duration
          }
        }
      });
    };

    setupDictationListeners();
    return () => {
      unlistenDictationStarted?.();
      unlistenDictationFinished?.();
      unlistenDictationPartialResult?.(); // Cleanup partial result listener
    };
  }, [barState, inputRef]); // barState needed for conditional, inputRef for focus attempt

  // Effect to listen for TTS lifecycle events
  useEffect(() => {
    let unlistenTtsStarted: (() => void) | undefined;
    let unlistenTtsFinished: (() => void) | undefined;

    const setupTtsListeners = async () => {
      unlistenTtsStarted = await listen<{ text: string }>(
        "app-tts-started",
        (event) => {
          console.log("FloatingBar Event: app-tts-started", event.payload);
          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
          setSpokenText(event.payload.text);
          setBarState("speaking");
        }
      );

      unlistenTtsFinished = await listen<null>("app-tts-finished", () => {
        console.log("FloatingBar Event: app-tts-finished");
        // Transition back to input or default. Input is likely more common.
        setBarState("input");
        setSpokenText("");
        requestAnimationFrame(() => {
          if (inputRef.current) inputRef.current.focus();
        });
      });
    };

    setupTtsListeners();

    return () => {
      unlistenTtsStarted?.();
      unlistenTtsFinished?.();
    };
  }, [inputRef]); // inputRef for focusing

  // Effect to emit bar state changes
  useEffect(() => {
    console.log("FloatingBar: Emitting bar-state-changed", barState);
    emit("bar-state-changed", { newState: barState }).catch(console.error);
  }, [barState]);

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
              console.log("Window focused: Expanding from default state.");
              handleBarClick(); // Use existing click handler to expand and focus
            } else if (barState === "input" && inputRef.current) {
              // If already in input state, ensure focus
              console.log(
                "Window focused: Ensuring input focus in 'input' state."
              );
              inputRef.current.focus();
            }
          } else {
            // When window loses focus
            if (barState === "input") {
              if (!inputValue.trim()) {
                // If in input state and input is empty, collapse by calling handleInputBlur.
                console.log(
                  "Window lost focus: Input is empty and state is 'input'. Calling handleInputBlur."
                );
                handleInputBlur();
              } else {
                // If in input state and input has text, do nothing to the bar state.
                // The input field will lose focus naturally. Bar remains expanded.
                console.log(
                  "Window lost focus: Input has text and state is 'input'. Bar remains expanded."
                );
              }
            } else {
              console.log(
                "Window lost focus: Bar not in 'input' state. No action."
              );
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

  // Effect for global mouseup to handle end of drag or click on bar
  useEffect(() => {
    const handleGlobalMouseUp = () => {
      if (isPreparingToDrag.current) {
        console.log("Global mouseup: Clearing isPreparingToDrag flag.");
        isPreparingToDrag.current = false;

        // After a potential drag, check if the bar should shrink
        if (
          barState === "input" &&
          !inputValue.trim() &&
          document.activeElement !== inputRef.current
        ) {
          console.log(
            "Global mouseup: Conditions to shrink met post-drag/click. Shrinking."
          );
          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
          setBarState("shrinking");
          transitionTimeoutRef.current = setTimeout(() => {
            setInputValue(""); // Clear input when shrinking back to default
            setBarState("default");
          }, 300); // Match the CSS transition duration
        }
      }
    };

    window.addEventListener("mouseup", handleGlobalMouseUp);
    return () => {
      window.removeEventListener("mouseup", handleGlobalMouseUp);
    };
  }, [barState, inputValue, setBarState, setInputValue]); // Dependencies for the effect

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
      default:
        return "h-[20px] w-[60px] px-2";
    }
  };

  return (
    <div
      data-window-hovered={isWindowHovered}
      className={cn(
        "w-screen h-screen flex items-start justify-start"
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
          data-tauri-drag-region
          className={cn(
            `
            flex items-center justify-center bg-black/90 backdrop-blur-md text-white
            rounded-full shadow-lg border border-white/20 overflow-hidden
            transition-all duration-300 ease-in-out
            [will-change:width,height]
            ${getBarStyles()}
            ${barState === "default" ? "cursor-pointer" : ""}
            `,
            // Add slight size increase on hover only when in default state
            barState === "default" && isWindowHovered && "scale-105"
          )}
          onClick={barState === "default" ? handleBarClick : undefined}
          onMouseDownCapture={(e) => {
            // If the bar is in input mode and the mousedown is on the bar itself (not its children like input/button)
            if (barState === "input" && e.target === e.currentTarget) {
              console.log(
                "Mousedown on bar (drag region) in input state. Setting isPreparingToDrag."
              );
              isPreparingToDrag.current = true;
            }
          }}
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
                onMouseDown={async (e) => {
                  if (barState === "input") {
                    console.log(
                      "Input MouseDown: Setting isPreparingToDrag and attempting to start window drag."
                    );
                    isPreparingToDrag.current = true;
                    e.preventDefault(); // Prevent text selection/focus stealing
                    try {
                      await Window.getCurrent().startDragging();
                      console.log("Window drag started via input.");
                    } catch (err) {
                      console.error(
                        "Failed to start dragging from input:",
                        err
                      );
                    }
                  }
                }}
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
              <Mic size={16} className="mr-2 text-white/70" />
              <span className="text-sm text-white/80">Listening...</span>
            </div>
          )}

          {/* Transcribing State Content */}
          {barState === "transcribing" && (
            <div className="w-full h-full flex items-center justify-start overflow-hidden px-3">
              <Mic size={16} className="mr-2 text-blue-400 flex-shrink-0" />
              <span className="text-sm text-white/90 truncate">
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
