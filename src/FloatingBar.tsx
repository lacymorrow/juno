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
      className={`w-screen h-screen bg-neutral-900 transition-all duration-300 ease-in-out overflow-hidden flex flex-col items-center justify-center px-4`} // Changed classes for full screen, added background, vertical centering
      style={
        {
          // Add styles for the gooey effect container if needed later
        }
      }
    >
      {/* Basic Input Form - now centered */}
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-md h-auto flex items-center bg-neutral-800/80 backdrop-blur-sm rounded-lg shadow-lg p-4 mb-4" // Added background, padding, max-width, margin
      >
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          // Removed onFocus/onBlur related to expansion as it's always "expanded" now
          placeholder={isLoading ? "Thinking..." : "Enter your command..."}
          disabled={isLoading}
          className={`w-full h-10 bg-transparent text-white placeholder-neutral-400 focus:outline-none px-2`} // Simplified classes, ensure padding
        />
        {!isLoading && ( // Show button if not loading
          <button
            type="submit"
            className="text-neutral-400 hover:text-white transition-colors ml-2 p-1"
            aria-label="Submit"
            disabled={!inputValue.trim()} // Disable if input is empty
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

      {/* Gooey Loading Indicator - Centered */}
      {isLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-neutral-900/50 pointer-events-none">
          {" "}
          {/* Adjusted background */}
          <GooeyLoader />
        </div>
      )}
    </div>
  );
};

export default FloatingBar;
