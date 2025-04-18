import React, { useEffect, useRef, useState } from "react";
import GooeyLoader from "./GooeyLoader"; // Import the loader
// import { invoke } from '@tauri-apps/api/core'; // Keep for later

const FloatingBar: React.FC = () => {
  const [inputValue, setInputValue] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false); // State to control expansion
  const inputRef = useRef<HTMLInputElement>(null); // Ref for the input element

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!inputValue.trim()) return;

    setIsLoading(true);
    console.log("Submitting:", inputValue);
    // TODO: Replace with invoke call to backend
    // try {
    //   const result = await invoke('submit_query', { query: inputValue });
    //   console.log('Backend response:', result);
    //   // Handle result - maybe display in main panel or notification
    // } catch (error) {
    //   console.error('Error submitting query:', error);
    // }
    setTimeout(() => {
      setIsLoading(false);
      setInputValue("");
      setIsExpanded(false); // Collapse after submit
      inputRef.current?.blur(); // Remove focus after submit
    }, 2000);
  };

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
