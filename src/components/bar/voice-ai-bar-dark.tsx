"use client";

import type React from "react";
import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useWindowSize } from "@/hooks/useWindowSize";
import {
  Mic,
  Volume2,
  AlertCircle,
  Send,
  Brain,
  Loader2,
  Check,
  Type,
  Sparkles,
} from "lucide-react";
import { cn } from "@/lib/utils";
import AudioVisualizer from "./audio-visualizer";
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../../src-tauri/tauri.conf.json";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

// Debounce utility
function debounce<T extends (...args: any[]) => any>(
  func: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: NodeJS.Timeout;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => func(...args), delay);
  };
}

// === STANDARDIZED UI API TYPES ===

/**
 * UI State enumeration - Uses generated constants from backend
 */
type UIState =
  | typeof UI.BAR_STATES_DEFAULT
  | typeof UI.BAR_STATES_EXPANDING
  | typeof UI.BAR_STATES_INPUT
  | typeof UI.BAR_STATES_SHRINKING
  | typeof UI.BAR_STATES_SUBMITTING
  | typeof UI.BAR_STATES_LOADING
  | typeof UI.BAR_STATES_FINISHING
  | typeof UI.BAR_STATES_SUCCESS
  | typeof UI.BAR_STATES_LISTENING
  | typeof UI.BAR_STATES_ERROR
  | typeof UI.BAR_STATES_TRANSCRIBING
  | typeof UI.BAR_STATES_SPEAKING
  | typeof UI.BAR_STATES_DICTATING
  | typeof UI.BAR_STATES_DICTATION_READY
  | typeof UI.BAR_STATES_ALWAYS_LISTENING
  | typeof UI.BAR_STATES_AGENT_RESPONDING;

/**
 * Backend State Data Structure
 */
interface BarStateData {
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  agentState: string | null;
}

/**
 * UI Interaction Event Structure
 */
interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

// === COMPONENT CONSTANTS ===

const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 100,
  DEFAULT_HEIGHT: 36,
  EXPANDED_WIDTH: 320,
  EXPANDED_HEIGHT: 48,
};

const COMPONENT_ID = "voice-ai-bar-dark";

// === MAIN COMPONENT ===

export function VoiceAIBarDark({ className = "" }: { className?: string }) {
  // === STATE MANAGEMENT ===

  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: UI.VOICE_MODES_IDLE,
    agentState: null,
  });

  const [localInputValue, setLocalInputValue] = useState("");
  const [isHovered, setIsHovered] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // === WINDOW CONFIGURATION ===

  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === "floating-bar"
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;

  // === BACKEND EVENT INTEGRATION ===

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            console.log("🌙 VoiceAIBarDark: Received state update:", event.payload);

            const payload = event.payload;
            if (
              payload &&
              typeof payload === "object" &&
              "barState" in payload
            ) {
              setBarState(payload);
            } else {
              console.error(
                "❌ VoiceAIBarDark: Invalid state data received:",
                payload
              );
            }
          }
        );

        console.log("✅ VoiceAIBarDark: Event listener established");
      } catch (error) {
        console.error("❌ VoiceAIBarDark: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      safeCleanupEventListener(unlisten);
      console.log("🔄 VoiceAIBarDark: Event listener cleaned up");
    };
  }, []);

  // === WINDOW RESIZING ===

  const lastSizeRef = useRef<{ width: number; height: number } | null>(null);
  const { resizeWindowIfChanged } = useWindowSize("floating-bar");
  const debouncedResizeWindow = useMemo(
    () => debounce(async (currentUiState: string) => {
      try {
        const appWindow = getCurrentWindow();

        const isCompact = [
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_DICTATION_READY,
        ].includes(currentUiState as any);
        
        const needsExpanded = [
          UI.BAR_STATES_INPUT,
          UI.BAR_STATES_EXPANDING,
          UI.BAR_STATES_AGENT_RESPONDING,
        ].includes(currentUiState as any);

        const currentWidth = needsExpanded 
          ? FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH 
          : isCompact 
          ? defaultWidth 
          : 240;
        const currentHeight = needsExpanded
          ? FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT
          : defaultHeight;

        await resizeWindowIfChanged({ width: currentWidth, height: currentHeight });
      } catch (error) {
        console.error("❌ VoiceAIBarDark: Failed to resize window:", error);
      }
    }, 100),
    []
  );

  useEffect(() => {
    debouncedResizeWindow(barState.barState);
  }, [barState.barState, debouncedResizeWindow]);

  // === INTERACTION HANDLERS ===

  const createInteraction = (
    interactionType: string,
    data?: Record<string, any>
  ): UIInteractionEvent => ({
    element_id: COMPONENT_ID,
    interaction_type: interactionType,
    data: data || null,
    timestamp: Date.now(),
  });

  const sendInteraction = async (interaction: UIInteractionEvent) => {
    try {
      console.log("🔧 VoiceAIBarDark: Sending interaction:", interaction);

      await invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction,
      });

      console.log("✅ VoiceAIBarDark: Interaction sent successfully");
    } catch (error) {
      console.error("❌ VoiceAIBarDark: Interaction failed:", error);
    }
  };

  // Sync local input with backend state
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  const handleClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const trimmedValue = localInputValue.trim();

      if (trimmedValue) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_SUBMIT, {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      }
    },
    [localInputValue]
  );

  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
  }, []);

  const handleFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS);
    await sendInteraction(interaction);
  }, []);

  const handleBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR);
    await sendInteraction(interaction);
  }, []);

  // === UI HELPERS ===

  const getStateIcon = () => {
    const currentUiState = barState.barState;
    switch (currentUiState) {
      case UI.BAR_STATES_LISTENING:
        return <Mic className="w-3.5 h-3.5 text-violet-400" />;
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return <Loader2 className="w-3.5 h-3.5 text-violet-400 animate-spin" />;
      case UI.BAR_STATES_SPEAKING:
        return <Volume2 className="w-3.5 h-3.5 text-emerald-400" />;
      case UI.BAR_STATES_ERROR:
        return <AlertCircle className="w-3.5 h-3.5 text-red-400" />;
      case UI.BAR_STATES_SUCCESS:
        return <Check className="w-3.5 h-3.5 text-emerald-400" />;
      case UI.BAR_STATES_INPUT:
        return <Sparkles className="w-3.5 h-3.5 text-violet-400" />;
      case UI.BAR_STATES_AGENT_RESPONDING:
        return <Brain className="w-3.5 h-3.5 text-violet-400 animate-pulse" />;
      case UI.BAR_STATES_TRANSCRIBING:
        return <Mic className="w-3.5 h-3.5 text-violet-400 animate-pulse" />;
      case UI.BAR_STATES_DICTATING:
        return <Type className="w-3.5 h-3.5 text-amber-400" />;
      case UI.BAR_STATES_DICTATION_READY:
        return <Type className="w-3.5 h-3.5 text-amber-400/60" />;
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return <Mic className="w-3.5 h-3.5 text-violet-400 animate-pulse" />;
      default:
        return <Brain className="w-3.5 h-3.5 text-gray-400" />;
    }
  };

  const getStateText = () => {
    switch (barState.barState) {
      case UI.BAR_STATES_LISTENING:
        return "Listening...";
      case UI.BAR_STATES_TRANSCRIBING:
        return "Converting speech...";
      case UI.BAR_STATES_SPEAKING:
        return barState.spokenText || "Speaking...";
      case UI.BAR_STATES_DICTATING:
        return "Dictating...";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return barState.agentState || "Agent thinking...";
      case UI.BAR_STATES_SUBMITTING:
        return "Sending...";
      case UI.BAR_STATES_LOADING:
        return "Processing...";
      case UI.BAR_STATES_ERROR:
        return barState.currentError || "Error occurred";
      case UI.BAR_STATES_SUCCESS:
        return "Complete!";
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return "Say 'Hey Juno'";
      case UI.BAR_STATES_DICTATION_READY:
        return "Dictation ready";
      default:
        return "Ready";
    }
  };

  const getBarClass = () => {
    const currentUiState = barState.barState;
    let baseClass = "dark-bar";

    const stateClasses = {
      [UI.BAR_STATES_LISTENING]: "dark-bar-listening",
      [UI.BAR_STATES_LOADING]: "dark-bar-processing",
      [UI.BAR_STATES_SUBMITTING]: "dark-bar-processing",
      [UI.BAR_STATES_SPEAKING]: "dark-bar-speaking",
      [UI.BAR_STATES_ERROR]: "dark-bar-error",
      [UI.BAR_STATES_SUCCESS]: "dark-bar-success",
      [UI.BAR_STATES_INPUT]: "dark-bar-input",
      [UI.BAR_STATES_AGENT_RESPONDING]: "dark-bar-agent",
      [UI.BAR_STATES_TRANSCRIBING]: "dark-bar-transcribing",
      [UI.BAR_STATES_DICTATING]: "dark-bar-dictating",
      [UI.BAR_STATES_ALWAYS_LISTENING]: "dark-bar-always-listening",
    };

    return cn(baseClass, stateClasses[currentUiState as keyof typeof stateClasses] || "dark-bar-default");
  };

  // === RENDER ===

  return (
    <div className={cn("voice-ai-bar-dark-container", className)}>
      <div
        className={getBarClass()}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {/* Input State */}
        {barState.barState === UI.BAR_STATES_INPUT && (
          <form onSubmit={handleSubmit} className="dark-input-form">
            <div className="dark-input-icon">{getStateIcon()}</div>
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => handleInputChange(e.target.value)}
              onFocus={handleFocus}
              onBlur={handleBlur}
              placeholder="Ask me anything..."
              className="dark-input"
              autoFocus
            />
            <button
              type="submit"
              className="dark-send-btn"
              disabled={!localInputValue.trim()}
            >
              <Send className="w-3 h-3" />
            </button>
          </form>
        )}

        {/* Default/Idle State */}
        {[UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
          barState.barState as any
        ) && (
          <div
            className="dark-default-content"
            onClick={handleClick}
          >
            <div className="dark-icon-wrapper">{getStateIcon()}</div>
            {isHovered && (
              <span className="dark-hint-text animate-fade-in">
                {barState.barState === UI.BAR_STATES_DICTATION_READY
                  ? "Ready to dictate"
                  : "Click or press Alt+D"}
              </span>
            )}
          </div>
        )}

        {/* Active States with Visualizer */}
        {[
          UI.BAR_STATES_LISTENING,
          UI.BAR_STATES_TRANSCRIBING,
          UI.BAR_STATES_SPEAKING,
          UI.BAR_STATES_LOADING,
          UI.BAR_STATES_SUBMITTING,
          UI.BAR_STATES_AGENT_RESPONDING,
        ].includes(barState.barState as any) && (
          <div className="dark-active-content">
            <div className="dark-icon-wrapper">{getStateIcon()}</div>
            <AudioVisualizer
              appState={
                barState.barState === UI.BAR_STATES_LISTENING
                  ? "listening"
                  : barState.barState === UI.BAR_STATES_SPEAKING
                  ? "speaking"
                  : "processing"
              }
              width={120}
              height={24}
              enableMicrophone={false}
              intensity={0.8}
              showTransitionProgress={false}
              animationStyle="minimal"
              className="dark-visualizer"
            />
            <span className="dark-status-text">{getStateText()}</span>
          </div>
        )}

        {/* Status States */}
        {[UI.BAR_STATES_ERROR, UI.BAR_STATES_SUCCESS].includes(
          barState.barState as any
        ) && (
          <div className="dark-status-content">
            <div className="dark-icon-wrapper">{getStateIcon()}</div>
            <span className="dark-status-text">{getStateText()}</span>
          </div>
        )}

        {/* Always Listening State */}
        {barState.barState === UI.BAR_STATES_ALWAYS_LISTENING && (
          <div className="dark-always-listening">
            <div className="dark-icon-wrapper">{getStateIcon()}</div>
            <div className="dark-listening-dots">
              <span className="dark-dot" />
              <span className="dark-dot" />
              <span className="dark-dot" />
            </div>
            <span className="dark-status-text">{getStateText()}</span>
          </div>
        )}
      </div>

      <style>{`
        .voice-ai-bar-dark-container {
          position: relative;
          height: 100%;
          width: 100%;
        }

        .dark-bar {
          position: relative;
          background: rgba(0, 0, 0, 0.8);
          backdrop-filter: blur(24px) saturate(200%);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: 12px;
          padding: 6px 12px;
          box-shadow: 
            0 8px 32px rgba(0, 0, 0, 0.4),
            inset 0 1px 0 rgba(255, 255, 255, 0.05);
          display: flex;
          align-items: center;
          height: 36px;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
          cursor: pointer;
        }

        .dark-bar:hover {
          background: rgba(0, 0, 0, 0.85);
          box-shadow: 
            0 12px 40px rgba(0, 0, 0, 0.5),
            inset 0 1px 0 rgba(255, 255, 255, 0.08);
        }

        .dark-bar-default {
          width: 100px;
        }

        .dark-bar-input {
          width: 320px;
          cursor: default;
        }

        .dark-bar-listening,
        .dark-bar-transcribing {
          width: 240px;
          border-color: rgba(139, 92, 246, 0.3);
          background: rgba(139, 92, 246, 0.05);
        }

        .dark-bar-processing {
          width: 200px;
          border-color: rgba(139, 92, 246, 0.2);
        }

        .dark-bar-speaking {
          width: 260px;
          border-color: rgba(16, 185, 129, 0.3);
          background: rgba(16, 185, 129, 0.05);
        }

        .dark-bar-error {
          width: 220px;
          border-color: rgba(239, 68, 68, 0.3);
          background: rgba(239, 68, 68, 0.05);
          animation: shake 0.5s ease-in-out;
        }

        .dark-bar-success {
          width: 180px;
          border-color: rgba(16, 185, 129, 0.3);
          background: rgba(16, 185, 129, 0.05);
        }

        .dark-bar-agent {
          width: 280px;
          border-color: rgba(139, 92, 246, 0.4);
          background: rgba(139, 92, 246, 0.08);
        }

        .dark-bar-always-listening {
          width: 160px;
          border-color: rgba(139, 92, 246, 0.2);
        }

        .dark-input-form {
          display: flex;
          align-items: center;
          gap: 10px;
          width: 100%;
        }

        .dark-input-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
        }

        .dark-input {
          flex: 1;
          background: transparent;
          border: none;
          outline: none;
          color: #f3f4f6;
          font-size: 14px;
          font-weight: 400;
        }

        .dark-input::placeholder {
          color: rgba(156, 163, 175, 0.6);
        }

        .dark-send-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 24px;
          height: 24px;
          background: rgba(139, 92, 246, 0.2);
          border: 1px solid rgba(139, 92, 246, 0.3);
          border-radius: 6px;
          color: #a78bfa;
          cursor: pointer;
          transition: all 0.2s ease;
          flex-shrink: 0;
        }

        .dark-send-btn:hover:not(:disabled) {
          background: rgba(139, 92, 246, 0.3);
          border-color: rgba(139, 92, 246, 0.5);
          transform: scale(1.05);
        }

        .dark-send-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .dark-default-content,
        .dark-active-content,
        .dark-status-content,
        .dark-always-listening {
          display: flex;
          align-items: center;
          gap: 8px;
          width: 100%;
        }

        .dark-icon-wrapper {
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
        }

        .dark-hint-text {
          font-size: 11px;
          color: rgba(156, 163, 175, 0.8);
          white-space: nowrap;
        }

        .dark-status-text {
          font-size: 12px;
          color: #e5e7eb;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .dark-visualizer {
          flex: 1;
          opacity: 0.8;
        }

        .dark-listening-dots {
          display: flex;
          gap: 3px;
          align-items: center;
        }

        .dark-dot {
          width: 3px;
          height: 3px;
          background: #a78bfa;
          border-radius: 50%;
          animation: pulse 1.5s ease-in-out infinite;
        }

        .dark-dot:nth-child(2) {
          animation-delay: 0.2s;
        }

        .dark-dot:nth-child(3) {
          animation-delay: 0.4s;
        }

        @keyframes shake {
          0%, 100% { transform: translateX(0); }
          25% { transform: translateX(-4px); }
          75% { transform: translateX(4px); }
        }

        @keyframes pulse {
          0%, 100% { opacity: 0.3; transform: scale(1); }
          50% { opacity: 1; transform: scale(1.2); }
        }

        @keyframes fade-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }

        .animate-fade-in {
          animation: fade-in 0.3s ease-out;
        }
      `}</style>
    </div>
  );
}