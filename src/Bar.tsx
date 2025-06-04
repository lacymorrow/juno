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
  | "speaking"
  | "dictating"; // New state for spacebar dictation

export function FloatingBar() {
  const [barState, setBarState] = useState<BarState>("default");
  const [inputValue, setInputValue] = useState("");
  const [lastSubmittedValue, setLastSubmittedValue] = useState("");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [transcriptionText, setTranscriptionText] = useState<string>("");
  const [spokenText, setSpokenText] = useState<string>("");
  const [isAgentWorking, setIsAgentWorking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const transitionTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const [isWindowHovered, setIsWindowHovered] = useState(false);
  const isPreparingToDrag = useRef(false); // Added: Flag for drag operation
  const [isAnimatingSize, setIsAnimatingSize] = useState(false); // For conditional backdrop-blur
  const [isSpacebarDictation, setIsSpacebarDictation] = useState(false); // Track spacebar vs AI agent mode

  // For debugging - log state changes
  useEffect(() => {
    console.log("Bar state changed to:", barState);
    if (barState === "expanding" || barState === "shrinking") {
      setIsAnimatingSize(true);
    } else {
      // Set to false when not in a direct size-changing animation state
      // This allows blur to be active in stable states like 'input', 'default', 'loading' etc.
      setIsAnimatingSize(false);
    }
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
          case "dictating":
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
    // Only allow expansion from default state - agent working states take precedence
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
    // Note: Working state checks are now handled at the caller level to preserve agent state priority
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
    let unlistenBackendResponse: (() => void) | undefined;
    let unlistenTimerExpired: (() => void) | undefined;

    const setupSubmitListeners = async () => {
      unlistenWillSubmit = await listen<{ query: string }>(
        "will-submit-query",
        (event) => {
          console.log("FloatingBar Event: will-submit-query", event.payload);
          const { query } = event.payload;
          setLastSubmittedValue(query);
          setInputValue(""); // Clear input field
          setCurrentError(null); // Clear any previous error
          setIsAgentWorking(true); // Agent starts working

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
          // Note: Don't transition to final states here anymore
          // Wait for backend-response to know when agent is actually done
          if (!event.payload.success) {
            // Only handle immediate submission errors
            setIsAgentWorking(false); // Agent is not working if submission failed
            setCurrentError(
              event.payload.error ||
                `Failed to submit: ${lastSubmittedValue}` ||
                "An unexpected error occurred during submission."
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
          // If success, keep loading state until backend-response
        }
      );

      // Listen for actual agent completion
      unlistenBackendResponse = await listen<{
        query: string;
        response: {
          text: string;
          agent_state: string;
          audio_base64?: string;
          screenshot_base64?: string;
        };
      }>("backend-response", (event) => {
        console.log("FloatingBar Event: backend-response", event.payload);
        const { response } = event.payload;

        // Agent is no longer working regardless of state
        setIsAgentWorking(false);

        if (transitionTimeoutRef.current) {
          clearTimeout(transitionTimeoutRef.current);
        }

        // Handle agent completion based on actual agent state
        if (response.agent_state === "Finished") {
          setBarState("finishing");
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("input");
            requestAnimationFrame(() => {
              if (inputRef.current) inputRef.current.focus();
            });
          }, 300); // Finishing duration
        } else if (
          response.agent_state === "Failed" ||
          response.agent_state === "Cancelled"
        ) {
          setCurrentError(
            response.agent_state === "Cancelled"
              ? "Agent execution was cancelled"
              : `Agent failed: ${response.text}`
          );
          setBarState("error");
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("input");
            setCurrentError(null); // Clear error after timeout
            requestAnimationFrame(() => {
              if (inputRef.current) inputRef.current.focus();
            });
          }, 3000); // Error state duration
        } else {
          // Other states - just transition to input for now
          setBarState("input");
          requestAnimationFrame(() => {
            if (inputRef.current) inputRef.current.focus();
          });
        }
      });

      unlistenTimerExpired = await listen<{
        id: string;
        description: string;
        context: any;
        trigger_time: number;
        created_at: number;
      }>("timer-expired", async (event) => {
        console.log("FloatingBar Event: timer-expired", event.payload);
        const { description, context } = event.payload;

        // Reset UI state
        setBarState("loading");
        setInputValue("");
        setCurrentError(null);
        setTranscriptionText("");
        setSpokenText("");

        if (transitionTimeoutRef.current) {
          clearTimeout(transitionTimeoutRef.current);
        }

        try {
          // Extract the resumption query from the context
          let resumeQuery = "";

          if (context.resumeQuery) {
            resumeQuery = context.resumeQuery;
          } else if (context.description) {
            resumeQuery = `Resume task: ${context.description}`;
          } else {
            resumeQuery = `Resume timer task: ${description}`;
          }

          // Add context information to the query
          if (context.gameState || context.taskState) {
            resumeQuery += ` Context: ${JSON.stringify(context)}`;
          }

          console.log("Restarting agent with timer context:", resumeQuery);

          // Mark agent as working when restarting
          setIsAgentWorking(true);

          // Restart the agent with the saved context
          await invoke("submit_query", {
            query: resumeQuery,
          });

          // Update UI to show success
          setBarState("success");
          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("shrinking");
            transitionTimeoutRef.current = setTimeout(() => {
              setBarState("default");
            }, 300);
          }, 2000);
        } catch (error) {
          console.error("Failed to restart agent from timer:", error);
          setIsAgentWorking(false); // Agent is not working if restart failed
          setCurrentError(`Failed to restart: ${error}`);
          setBarState("error");

          transitionTimeoutRef.current = setTimeout(() => {
            setBarState("shrinking");
            transitionTimeoutRef.current = setTimeout(() => {
              setBarState("default");
            }, 300);
          }, 3000);
        }
      });
    };

    setupSubmitListeners();
    return () => {
      unlistenWillSubmit?.();
      unlistenDidSubmit?.();
      unlistenBackendResponse?.();
      unlistenTimerExpired?.();
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
          // Handle partial results for both listening/transcribing and dictating states
          if (
            barState === "listening" ||
            barState === "transcribing" ||
            barState === "dictating"
          ) {
            setTranscriptionText(event.payload.partial);
            // Update state to show transcribing unless we're in spacebar dictation mode
            if (barState === "listening") {
              setBarState(isSpacebarDictation ? "dictating" : "transcribing");
            }
          }
        }
      );

      unlistenDictationFinished = await listen<{
        query: string | null;
        error?: string;
      }>("app-dictation-finished", (event) => {
        console.log("FloatingBar Event: app-dictation-finished", event.payload);
        setTranscriptionText(""); // Clear transcription on finish

        if (
          barState === "listening" ||
          barState === "transcribing" ||
          barState === "dictating"
        ) {
          // Only act if we were in a dictation state
          if (transitionTimeoutRef.current)
            clearTimeout(transitionTimeoutRef.current);
          if (event.payload.query) {
            // A query was successfully dictated.
            // For spacebar dictation, we don't show input field, just process directly
            if (isSpacebarDictation) {
              // Spacebar dictation is handled differently - text is typed directly
              setBarState("finishing");
              transitionTimeoutRef.current = setTimeout(() => {
                setBarState("default");
              }, 500);
            } else {
              // Regular dictation - Display the transcribed text in the input field
              setInputValue(event.payload.query);
              setBarState("input");
              requestAnimationFrame(() => {
                if (inputRef.current && event.payload.query) {
                  inputRef.current.focus();
                  // Place cursor at the end of the transcribed text
                  inputRef.current.setSelectionRange(
                    event.payload.query.length,
                    event.payload.query.length
                  );
                }
              });
            }
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

  // Effect to listen for spacebar dictation events to differentiate from AI agent mode
  useEffect(() => {
    let unlistenSpacebarActive: (() => void) | undefined;
    let unlistenSpacebarStart: (() => void) | undefined;
    let unlistenSpacebarStop: (() => void) | undefined;

    const setupSpacebarListeners = async () => {
      unlistenSpacebarActive = await listen<boolean>(
        "spacebar-dictation-active",
        (event) => {
          console.log(
            "FloatingBar Event: spacebar-dictation-active",
            event.payload
          );
          setIsSpacebarDictation(event.payload);

          // Set visual state based on spacebar dictation status
          if (event.payload) {
            setBarState("dictating");
            if (transitionTimeoutRef.current) {
              clearTimeout(transitionTimeoutRef.current);
            }
          }
        }
      );

      // Listen for immediate transcription start events
      unlistenSpacebarStart = await listen(
        "spacebar-transcription-start",
        () => {
          console.log(
            "FloatingBar Event: spacebar-transcription-start (immediate)"
          );
          setIsSpacebarDictation(true);
          setBarState("dictating");
          setTranscriptionText(""); // Clear any previous transcription
          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
        }
      );

      // Listen for dictation commitment (threshold reached)
      const unlistenSpacebarCommitted = await listen(
        "spacebar-dictation-committed",
        () => {
          console.log(
            "FloatingBar Event: spacebar-dictation-committed (threshold reached)"
          );
          // User has committed to dictation - we can show additional UI feedback here if desired
          // The "dictating" state is already set, so this is just for additional feedback
        }
      );

      // Listen for transcription cancellation events
      const unlistenSpacebarCancel = await listen(
        "spacebar-transcription-cancel",
        () => {
          console.log("FloatingBar Event: spacebar-transcription-cancel");
          setIsSpacebarDictation(false);

          // Quickly return to default state since this was cancelled
          setBarState("default");
          setTranscriptionText("");

          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
        }
      );

      // Listen for spacebar dictation stop events (normal completion)
      unlistenSpacebarStop = await listen("spacebar-dictation-stop", () => {
        console.log(
          "FloatingBar Event: spacebar-dictation-stop (normal completion)"
        );
        setIsSpacebarDictation(false);

        // Briefly show a completion state, then return to default
        setBarState("finishing");
        setTranscriptionText("");

        transitionTimeoutRef.current = setTimeout(() => {
          setBarState("default");
        }, 500); // Show finishing state briefly
      });

      // Listen for force stop events (emergency cleanup)
      const unlistenForceStop = await listen(
        "spacebar-transcription-force-stop",
        () => {
          console.warn(
            "FloatingBar Event: spacebar-transcription-force-stop - emergency cleanup"
          );
          setIsSpacebarDictation(false);
          setBarState("default");
          setTranscriptionText("");

          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
        }
      );

      // Listen for force cleanup events (stuck state recovery)
      const unlistenForceCleanup = await listen(
        "spacebar-transcription-force-cleanup",
        () => {
          console.warn(
            "FloatingBar Event: spacebar-transcription-force-cleanup - recovering stuck state"
          );
          setIsSpacebarDictation(false);
          setBarState("default");
          setTranscriptionText("");

          if (transitionTimeoutRef.current) {
            clearTimeout(transitionTimeoutRef.current);
          }
        }
      );

      // Return cleanup functions for new listeners
      return () => {
        unlistenSpacebarCommitted?.();
        unlistenSpacebarCancel?.();
        unlistenForceStop?.();
        unlistenForceCleanup?.();
      };
    };

    const spacebarCleanup = setupSpacebarListeners();
    return () => {
      unlistenSpacebarActive?.();
      unlistenSpacebarStart?.();
      unlistenSpacebarStop?.();
      spacebarCleanup?.then((cleanup) => cleanup?.());
    };
  }, []);

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

  // Helper function to determine if the bar should remain expanded for status display
  const shouldRemainExpandedForStatus = (state: BarState): boolean => {
    const workingStates: BarState[] = [
      "loading",
      "finishing",
      "success",
      "speaking",
      "listening",
      "transcribing",
      "dictating",
      "error",
    ];
    return workingStates.includes(state) || isAgentWorking;
  };

  // Effect to handle window focus changes
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      const currentWindow = Window.getCurrent();
      unlisten = await currentWindow.onFocusChanged(
        ({ payload: isFocused }) => {
          console.log(
            "Window focus changed:",
            isFocused,
            "Current bar state:",
            barState
          );

          // Never change bar state if agent is working - agent state takes precedence
          if (shouldRemainExpandedForStatus(barState)) {
            console.log(
              "Window focus changed: Agent is working, no bar state changes allowed"
            );
            if (isFocused && barState === "input" && inputRef.current) {
              // Still allow input focus when window gains focus, but don't change bar state
              inputRef.current.focus();
            }
            return;
          }

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
            // When window loses focus - only handle if agent is idle
            if (barState === "input" && !inputValue.trim()) {
              console.log(
                "Window lost focus: Input is empty and agent is idle. Shrinking bar."
              );
              handleInputBlur();
            } else {
              console.log(
                "Window lost focus: Bar state preserved (has content or agent active)."
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
  }, [barState, inputValue, isAgentWorking, handleBarClick, handleInputBlur]);

  // Effect for global mouseup to handle end of drag or click on bar
  useEffect(() => {
    const handleGlobalMouseUp = () => {
      if (isPreparingToDrag.current) {
        console.log("Global mouseup: Clearing isPreparingToDrag flag.");
        isPreparingToDrag.current = false;

        // Never change bar state if agent is working - agent state takes precedence
        if (shouldRemainExpandedForStatus(barState)) {
          console.log(
            `Global mouseup: Agent is working (${barState}), preserving bar state.`
          );
          return;
        }

        // After a potential drag, check if the bar should shrink (only when agent is idle)
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
  }, [barState, inputValue, isAgentWorking, setBarState, setInputValue]); // Dependencies for the effect

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
      case "dictating":
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
      <div className="relative z-50 p-3">
        {/* Universal Bar Container - Now positioned within the flex container */}
        <div
          data-tauri-drag-region
          className={cn(
            `
            flex items-center justify-center bg-black/90 text-white
            rounded-full shadow-lg border border-white/20 overflow-hidden
            transition-all duration-300 ease-in-out
            [will-change:width,height,transform]
            [backface-visibility:hidden]
            [transform-origin:center]
            ${getBarStyles()}
            ${barState === "default" ? "cursor-pointer" : ""}
            `,
            !isAnimatingSize && "backdrop-blur-md", // Conditionally apply backdrop-blur
            // Add slight size increase on hover only when in default state with optimized transform
            barState === "default" &&
              isWindowHovered &&
              "[transform:scale3d(1.05,1.05,1)]"
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

          {/* Dictating State Content - Spacebar hold-to-dictate */}
          {barState === "dictating" && (
            <div className="w-full h-full flex items-center justify-start overflow-hidden px-3 animate-pulse">
              <div className="mr-2 flex-shrink-0 relative">
                <Mic size={16} className="text-orange-400" />
                <div className="absolute -top-1 -right-1 w-2 h-2 bg-orange-400 rounded-full animate-ping"></div>
              </div>
              <span className="text-sm text-orange-200 truncate font-medium">
                {transcriptionText || "Hold spacebar to dictate..."}
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
