import { invoke } from "@tauri-apps/api/core"; // Import invoke
import { appWindow } from "@tauri-apps/api/window"; // Import appWindow
import React, { useEffect, useRef, useState } from "react";
import GooeyLoader from "./GooeyLoader"; // Import the loader

// Type for the result from submit_query (copied from App.tsx)
type SubmitQueryResult = {
  text: string;
  audio_base64?: string; // Optional base64 audio data
};

// Type for conversation messages (optional, if storing conversation here)
// type ChatMessage = {
//   role: "user" | "assistant" | "system";
//   content: string;
// };

const FloatingBar: React.FC = () => {
  const [inputValue, setInputValue] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false); // State to control expansion
  const inputRef = useRef<HTMLInputElement>(null); // Ref for the input element
  // Optional: State to hold the last response text
  // const [lastResponse, setLastResponse] = useState<string | null>(null);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!inputValue.trim() || isLoading) return;

    const query = inputValue;
    setIsLoading(true);
    setInputValue(""); // Clear input immediately
    inputRef.current?.blur(); // Blur input
    // Keep it expanded while loading
    // setIsExpanded(false); // Don't collapse immediately

    console.log("Submitting query:", query);

    try {
      // Call the backend invoke function
      const result: SubmitQueryResult = await invoke("submit_query", {
        query: query,
      });
      console.log("Backend response:", result);
      // TODO: Handle the result (e.g., show notification, play audio, update main window?)
      // Example: setLastResponse(result.text);
      // If audio, could potentially play it here too
      // playAudioFromBase64(result.audio_base64);
    } catch (error) {
      console.error("Error submitting query:", error);
      // TODO: Show error state to user?
      // Example: setLastResponse(`Error: ${error}`);
    } finally {
      setIsLoading(false);
      // Collapse only after loading is finished (success or error)
      setIsExpanded(false);
      // Optionally clear last response after a delay?
      // setTimeout(() => setLastResponse(null), 5000);
    }
  };

  // Effect to handle window focus changes
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await appWindow.onFocusChanged(({ payload: isFocused }) => {
        console.log("Window focus changed:", isFocused);
        if (isFocused) {
          // When window gains focus, focus the input
          inputRef.current?.focus();
          // Optionally expand the bar when window is focused
          // setIsExpanded(true);
        } else {
          // When window loses focus, blur the input and potentially collapse
          inputRef.current?.blur();
          if (!inputValue) {
            // Only collapse if input is empty
            setIsExpanded(false);
          }
        }
      });
    };

    setupListener();

    // Cleanup listener on component unmount
    return () => {
      unlisten?.();
    };
  }, [inputValue]); // Re-run if inputValue changes to update collapse logic on blur

  const handleFocus = () => {
    setIsExpanded(true);
  };

  const handleBlur = (event: React.FocusEvent<HTMLInputElement>) => {
    // Delay collapse slightly to allow clicking submit button
    setTimeout(() => {
      // Check if the new focused element is *not* the submit button or the input itself
      // This check might need refinement depending on exact behavior desired.
      // For now, collapse if input is empty.
      if (!inputValue && document.activeElement !== inputRef.current) {
        setIsExpanded(false);
      }
      // A more robust check might involve relatedTarget if supported well
      // if (!event.relatedTarget || !(event.currentTarget.contains(event.relatedTarget as Node))) {
      //    if (!inputValue) setIsExpanded(false);
      // }
    }, 150); // Small delay
  };

  // Effect to manage focus based on isLoading state (optional refinement)
  useEffect(() => {
    if (!isLoading && isExpanded && inputRef.current) {
      // Maybe re-focus input if needed, or handle differently
      // Check if the window is actually focused before focusing input
      appWindow.isFocused().then((isFocused) => {
        if (isFocused) {
          inputRef.current?.focus();
        }
      });
    } else if (isLoading && inputRef.current) {
      inputRef.current.blur(); // Blur input when loading starts
    }
  }, [isLoading, isExpanded]);

  return (
    <div
      data-tauri-drag-region // Make the entire container draggable
      className={`
        w-screen h-screen flex items-center justify-center
        transition-all duration-300 ease-in-out
        ${
          isExpanded || isLoading
            ? "bg-neutral-900/80 backdrop-blur-sm"
            : "bg-transparent"
        }
      `}
    >
      <div
        data-tauri-drag-region // Make the entire container draggable
        className={`
          transition-all duration-300 ease-in-out overflow-hidden rounded-lg shadow-xl
          w-full max-w-md
          ${
            isExpanded || isLoading
              ? "h-auto p-3 bg-neutral-800/90"
              : "h-12 p-0 bg-neutral-800/50"
          } // Dynamic height, padding, background
          relative // Needed for absolute positioning of loader
        `}
      >
        {/* Input Form */}
        <form
          data-tauri-drag-region // Make the entire container draggable
          onSubmit={handleSubmit}
          className={`w-full flex items-center transition-opacity duration-200 ease-in-out ${
            isLoading ? "opacity-0" : "opacity-100"
          }`} // Fade out form when loading
          style={{ height: "48px" }} // Fixed height for the form itself
        >
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onFocus={handleFocus}
            onBlur={handleBlur} // Use the refined blur handler
            placeholder={
              isLoading ? "" : isExpanded ? "Enter your command..." : "..."
            } // Dynamic placeholder
            disabled={isLoading}
            className={`
              flex-grow h-full bg-transparent text-white placeholder-neutral-400
              focus:outline-none px-3 transition-all duration-300 ease-in-out
              ${
                isExpanded || isLoading ? "text-base" : "text-sm"
              } // Smaller text when collapsed
            `}
          />
          {(isExpanded || inputValue) &&
            !isLoading && ( // Show button if expanded or has input (and not loading)
              <button
                type="submit"
                className="text-neutral-400 hover:text-white transition-colors ml-2 p-2 flex-shrink-0" // Ensure button doesn't shrink input
                aria-label="Submit"
                disabled={!inputValue.trim()} // Disable if input is empty
              >
                {/* Simple Arrow Icon */}
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M5 12h14" />
                  <path d="m12 5 7 7-7 7" />
                </svg>
              </button>
            )}
        </form>

        {/* Gooey Loading Indicator - Centered within the inner div */}
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center bg-neutral-800/80 pointer-events-auto">
            <GooeyLoader />
          </div>
        )}
      </div>
    </div>
  );
};

export default FloatingBar;
