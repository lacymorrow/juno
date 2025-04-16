import React, { useState } from "react";
import GooeyLoader from "./GooeyLoader"; // Import the loader
// import { invoke } from '@tauri-apps/api/core'; // Keep for later

const FloatingBar: React.FC = () => {
  const [inputValue, setInputValue] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false); // State to control expansion

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
    // Simulate loading finished
    setTimeout(() => {
      setIsLoading(false);
      setInputValue("");
      setIsExpanded(false); // Collapse after submit
    }, 2000);
  };

  const handleFocus = () => {
    setIsExpanded(true);
  };

  // Optional: Collapse when focus is lost, might need careful handling
  // const handleBlur = () => {
  //   if (!inputValue) { // Only collapse if input is empty
  //      setIsExpanded(false);
  //   }
  // };

  return (
    <div
      data-tauri-drag-region // Make the bar draggable
      className={`fixed bottom-10 left-1/2 -translate-x-1/2 w-[350px] h-[40px] bg-neutral-800/80 backdrop-blur-sm rounded-lg shadow-lg transition-all duration-300 ease-in-out overflow-hidden flex items-center px-2 ${
        isExpanded ? "!h-[80px]" : ""
      }`} // Start smaller, expand on focus
      style={
        {
          // Add styles for the gooey effect container if needed later
        }
      }
    >
      {/* Basic Input Form - expand on focus */}
      <form onSubmit={handleSubmit} className="w-full h-full flex items-center">
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onFocus={handleFocus}
          // onBlur={handleBlur} // Enable if blur-to-collapse is desired
          placeholder={isLoading ? "Thinking..." : "Enter your command..."}
          disabled={isLoading}
          className={`w-full h-full bg-transparent text-white placeholder-neutral-400 focus:outline-none transition-opacity duration-300 ${
            !isExpanded && !isLoading
              ? "opacity-0 pointer-events-none"
              : "opacity-100"
          }`}
        />
        {/* Show submit icon only when expanded and not loading */}
        {isExpanded && !isLoading && (
          <button
            type="submit"
            className="text-neutral-400 hover:text-white transition-colors ml-2 p-1"
            aria-label="Submit"
          >
            {/* Basic Send Icon (replace with actual icon later) */}
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

      {/* Gooey Loading Indicator */}
      {isLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-neutral-800/90 pointer-events-none">
          <GooeyLoader />
        </div>
      )}
    </div>
  );
};

export default FloatingBar;
