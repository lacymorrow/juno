"use client"
import { Settings } from "lucide-react"
import type { DevPanelProps, AssistantState } from "../types/voice-ai"

export function DevPanel({
  currentState,
  onStateChange,
  sampleResponses,
  isVisible,
  onToggleVisibility,
}: DevPanelProps) {
  const handleStateChange = (state: AssistantState) => {
    if (state === "response") {
      // Set sample response data when switching to response state
      // This is handled by the parent component through the callback
    }
    onStateChange(state)
  }

  return (
    <>
      {/* Dev Panel Toggle */}
      <button
        onClick={onToggleVisibility}
        className="fixed top-4 right-4 p-2 bg-gray-800 rounded-full opacity-50 hover:opacity-100 transition-opacity z-50"
      >
        <Settings className="w-5 h-5 text-white" />
      </button>

      {/* Dev Panel */}
      {isVisible && (
        <div className="fixed top-16 right-4 bg-gray-800 p-4 rounded-lg shadow-lg z-50 text-white">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-medium">Dev Controls</h3>
            <span className="text-xs bg-purple-600 px-2 py-0.5 rounded">DEV ONLY</span>
          </div>
          <div className="space-y-2">
            {(
              [
                "idle",
                "listening",
                "processing",
                "speaking",
                "error",
                "success",
                "input",
                "response",
              ] as AssistantState[]
            ).map((state) => (
              <button
                key={state}
                onClick={() => handleStateChange(state)}
                className={`block w-full text-left px-3 py-1.5 rounded text-sm capitalize transition-colors ${
                  currentState === state ? "bg-purple-700" : "bg-gray-700 hover:bg-gray-600"
                }`}
              >
                {state}
              </button>
            ))}
            {currentState === "response" && (
              <div className="mt-2 pt-2 border-t border-gray-600">
                <span className="text-xs text-gray-400">Response controls handled by bar</span>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  )
}
