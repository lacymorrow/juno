import React from "react";
import { cn } from "@/lib/utils";
import {
  AlertCircle,
  Brain,
  Check,
  Loader2,
  Mic,
  MicOff,
  Send,
  Sparkles,
  Type,
  Volume2,
  X,
} from "lucide-react";
import type { BarState } from "@/types/floating-bar";

interface FloatingBarStatesProps {
  barState: BarState;
  transcriptionText: string;
  spokenText: string;
  currentError: string | null;
  lastSubmittedValue: string;
  inputValue: string;
  audioLevel: number;
  onInputChange: (value: string) => void;
  onInputBlur: () => void;
  onSubmit: (e: React.FormEvent) => void;
  inputRef: React.RefObject<HTMLInputElement>;
}

// Get main icon based on enhanced state
export function getMainIcon(barState: BarState) {
  switch (barState) {
    case "default":
      return <Sparkles className="h-4 w-4 text-emerald-400" />;
    case "dictation_ready":
      return <MicOff className="h-4 w-4 text-muted-foreground" />;
    case "dictation_active":
    case "dictating":
      return <Type className="h-4 w-4 text-orange-500" />;
    case "dictation_processing":
    case "transcribing":
      return <Loader2 className="h-4 w-4 text-orange-500 animate-spin" />;
    case "agent_listening":
    case "listening":
      return <Brain className="h-4 w-4 text-blue-500" />;
    case "agent_thinking":
      return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
    case "agent_responding":
      return <Brain className="h-4 w-4 text-blue-500 animate-pulse" />;
    case "always-listening":
      return <Mic className="h-4 w-4 text-blue-400" />;
    case "speaking":
      return <Volume2 className="h-4 w-4 text-purple-500" />;
    case "loading":
      return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
    case "success":
      return <Check className="h-4 w-4 text-emerald-500" />;
    case "error":
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    default:
      return <Mic className="h-4 w-4 text-blue-500" />;
  }
}

// Audio level visualization component
function AudioLevelIndicator({
  barState,
  audioLevel,
}: {
  barState: BarState;
  audioLevel: number;
}) {
  if (
    !["dictation_active", "dictating", "agent_listening", "listening"].includes(
      barState
    )
  )
    return null;

  return (
    <div
      className="flex items-center gap-1 ml-2 cursor-move"
      data-tauri-drag-region
    >
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          data-tauri-drag-region
          className={cn(
            "w-1 rounded-full transition-all duration-150",
            audioLevel > (i + 1) * 20 ? "bg-white h-3" : "bg-white/30 h-1"
          )}
        />
      ))}
    </div>
  );
}

export function FloatingBarStates({
  barState,
  transcriptionText,
  spokenText,
  currentError,
  lastSubmittedValue,
  inputValue,
  audioLevel,
  onInputChange,
  onInputBlur,
  onSubmit,
  inputRef,
}: FloatingBarStatesProps) {
  // Default State
  if (
    barState === "default" ||
    barState === "dictation_ready" ||
    barState === "finishing"
  ) {
    return (
      <div className="flex items-center gap-2" data-tauri-drag-region>
        {getMainIcon(barState)}
      </div>
    );
  }

  // Expanding/Input State
  if (barState === "expanding" || barState === "input") {
    return (
      <form
        data-tauri-drag-region
        onSubmit={onSubmit}
        className={cn(
          "flex items-center justify-between w-full h-full gap-3",
          "transition-opacity duration-300 ease-in-out",
          barState === "input" ? "opacity-100" : "opacity-0"
        )}
      >
        <div className="flex items-center gap-2 flex-1" data-tauri-drag-region>
          {getMainIcon(barState)}
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onChange={(e) => onInputChange(e.target.value)}
            onBlur={onInputBlur}
            placeholder="Ask me anything..."
            className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/60"
            disabled={barState !== "input"}
          />
        </div>
        <button
          data-tauri-drag-region
          type="submit"
          className="text-white/60 hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200"
          disabled={barState !== "input"}
        >
          <Send size={14} />
        </button>
      </form>
    );
  }

  // Enhanced Voice States
  if (
    [
      "dictation_active",
      "dictation_processing",
      "dictating",
      "transcribing",
      "agent_listening",
      "agent_thinking",
      "agent_responding",
      "listening",
    ].includes(barState)
  ) {
    return (
      <div
        className="flex items-center justify-between w-full h-full"
        data-tauri-drag-region
      >
        <div
          className="flex items-center gap-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          {getMainIcon(barState)}
          <div className="flex-1 min-w-0" data-tauri-drag-region>
            <div
              className="text-sm font-medium truncate"
              data-tauri-drag-region
            >
              {getStatusText(barState, currentError)}
            </div>
            {transcriptionText && (
              <div
                className="text-xs text-white/70 truncate"
                data-tauri-drag-region
              >
                "{transcriptionText}"
              </div>
            )}
          </div>
        </div>
        <AudioLevelIndicator barState={barState} audioLevel={audioLevel} />
      </div>
    );
  }

  // Always Listening State
  if (barState === "always-listening") {
    return (
      <div
        className="flex items-center justify-between w-full h-full"
        data-tauri-drag-region
      >
        <div
          className="flex items-center gap-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          <Mic className="h-4 w-4 text-blue-400 animate-pulse" />
          <span
            className="text-sm text-blue-200 truncate font-medium"
            data-tauri-drag-region
          >
            Always listening for wake words...
          </span>
        </div>
        <div className="flex items-center gap-1 ml-2" data-tauri-drag-region>
          <div
            className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
            data-tauri-drag-region
          />
          <div
            className="w-1 h-2 bg-blue-300 rounded-full animate-pulse"
            style={{ animationDelay: "0.1s" }}
            data-tauri-drag-region
          />
          <div
            className="w-1 h-1 bg-blue-400 rounded-full animate-pulse"
            style={{ animationDelay: "0.2s" }}
            data-tauri-drag-region
          />
        </div>
      </div>
    );
  }

  // Speaking State
  if (barState === "speaking") {
    return (
      <div
        className="flex items-center justify-between w-full h-full"
        data-tauri-drag-region
      >
        <div
          className="flex items-center gap-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          <Volume2 className="h-4 w-4 text-purple-300 animate-pulse" />
          <span
            className="text-sm text-white/90 truncate"
            data-tauri-drag-region
          >
            {spokenText || "Playing response..."}
          </span>
        </div>
      </div>
    );
  }

  // Loading State
  if (barState === "loading") {
    return (
      <div
        className="flex flex-col items-center justify-center w-full h-full gap-2"
        data-tauri-drag-region
      >
        <div className="flex items-center gap-2" data-tauri-drag-region>
          <Loader2 className="h-4 w-4 animate-spin" />
          <span className="text-sm font-medium" data-tauri-drag-region>
            Processing
          </span>
        </div>
        {lastSubmittedValue && (
          <div
            className="text-xs text-white/70 truncate w-full text-center"
            data-tauri-drag-region
          >
            {lastSubmittedValue}
          </div>
        )}
      </div>
    );
  }

  // Success State
  if (barState === "success") {
    return (
      <div
        className="flex items-center justify-between w-full h-full animate-success-fade"
        data-tauri-drag-region
      >
        <div
          className="flex items-center gap-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          <Check className="h-4 w-4 text-emerald-300" />
          <span
            className="text-sm font-medium text-emerald-100 truncate"
            data-tauri-drag-region
          >
            {lastSubmittedValue}
          </span>
        </div>
        <div
          className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-400"
          data-tauri-drag-region
        >
          <Check size={12} className="text-emerald-900" />
        </div>
      </div>
    );
  }

  // Error State
  if (barState === "error") {
    return (
      <div
        className="flex items-center justify-between w-full h-full"
        data-tauri-drag-region
      >
        <div
          className="flex items-center gap-3 flex-1 min-w-0"
          data-tauri-drag-region
        >
          <AlertCircle className="h-4 w-4 text-red-300" />
          <span
            className="text-sm font-medium text-red-100 truncate"
            data-tauri-drag-region
          >
            {currentError || "Error occurred"}
          </span>
        </div>
        <div
          className="flex items-center justify-center h-6 w-6 rounded-full bg-red-400"
          data-tauri-drag-region
        >
          <X size={12} className="text-red-900" />
        </div>
      </div>
    );
  }

  // Shrinking State
  if (barState === "shrinking") {
    return (
      <div
        className="opacity-0 w-full h-full transition-opacity duration-300"
        data-tauri-drag-region
      />
    );
  }

  // Fallback
  return null;
}

// Get enhanced status text for tooltip
function getStatusText(barState: BarState, currentError: string | null) {
  switch (barState) {
    case "dictation_ready":
      return "Hold Option+Space to start dictating";
    case "dictation_active":
    case "dictating":
      return "Dictating... Release key to finish";
    case "dictation_processing":
    case "transcribing":
      return "Processing dictation...";
    case "agent_listening":
    case "listening":
      return "Listening for voice command...";
    case "agent_thinking":
      return "AI is thinking...";
    case "agent_responding":
      return "AI is responding...";
    case "speaking":
      return "Playing AI response";
    case "loading":
      return "Processing request...";
    case "success":
      return "Task completed successfully";
    case "error":
      return currentError || "An error occurred";
    case "always-listening":
      return "Always listening for wake words";
    default:
      return "Voice assistant ready";
  }
}
