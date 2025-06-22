import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import {
  ChevronUp,
  Minimize2,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  useVoice,
  useAgentState,
  useRecentMessages,
} from "@/contexts/VoiceContext";
import { VoiceAIBar } from "./voice-ai-bar";
import type { AssistantState, ResponseContent } from "../types/voice-ai";

// Enhanced state management for the transparent panel
interface PanelState {
  mode: "compact" | "expanded" | "chat" | "settings";
  agentStatus: "idle" | "listening" | "thinking" | "responding" | "error";
  currentResponse?: string;
  error?: string;
  voiceMode?: "dictation" | "agent";
  isSpeaking?: boolean;
  transcriptionText?: string;
}

interface TransparentFloatingPanelProps {
  className?: string;
  maxWidth?: number;
  opacity?: number;
  isWindowHovered?: boolean;
  disableWindowManagement?: boolean;
  onModeChange?: (mode: "compact" | "expanded" | "chat" | "settings") => void;
}

// Map FloatingBar states to VoiceAIBar states
const mapBarStateToAssistantState = (
  panelState: PanelState,
  voiceState: any
): AssistantState => {
  if (panelState.error) return "error";
  if (voiceState.isSpeaking) return "speaking";
  if (panelState.agentStatus === "responding") return "response";
  if (panelState.agentStatus === "thinking") return "processing";
  if (voiceState.isListening) return "listening";
  if (voiceState.isTranscribing) return "processing";
  if (panelState.mode === "chat" && panelState.currentResponse) return "success";
  return "idle";
};

// Generate response content based on current state
const generateResponseContent = (
  panelState: PanelState,
  voiceState: any,
  recentMessages: any[]
): Record<string, ResponseContent> => {
  const responses: Record<string, ResponseContent> = {};

  if (panelState.currentResponse) {
    responses.current = {
      type: "text",
      title: "Current Response",
      content: panelState.currentResponse,
    };
  }

  if (voiceState.transcriptionText) {
    responses.transcription = {
      type: "text",
      title: "Transcription",
      content: `"${voiceState.transcriptionText}"`,
    };
  }

  if (recentMessages.length > 0) {
    const lastMessage = recentMessages[recentMessages.length - 1];
    responses.lastMessage = {
      type: "text",
      title: "Recent Message",
      content: lastMessage.content,
    };
  }

  // Default sample content
  if (Object.keys(responses).length === 0) {
    responses.default = {
      type: "text",
      title: "Juno AI Assistant",
      content: "I'm ready to help you with computer tasks, voice commands, and general assistance. Try speaking to me or typing a message!",
    };
  }

  return responses;
};

// Integrated VoiceAIBar component that bridges the gap
const IntegratedVoiceAIBar = ({
  panelState,
  voiceState,
  recentMessages,
  onSubmit,
  inputValue,
  setInputValue,
  inputRef,
}: {
  panelState: PanelState;
  voiceState: any;
  recentMessages: any[];
  onSubmit: (e: FormEvent) => void;
  inputValue: string;
  setInputValue: (value: string) => void;
  inputRef: React.RefObject<HTMLInputElement>;
}) => {
  const assistantState = mapBarStateToAssistantState(panelState, voiceState);
  const sampleResponses = generateResponseContent(panelState, voiceState, recentMessages);

  const handleStateChange = (newState: AssistantState) => {
    // Handle state changes that affect the backend
    console.log("VoiceAIBar state changed to:", newState);
  };

  return (
    <div className="relative w-full h-full">
      <VoiceAIBar
        onStateChange={handleStateChange}
        initialState={assistantState}
        sampleResponses={sampleResponses}
        className="w-full h-full"
      />

      {/* Overlay input handling for input state */}
      {assistantState === "input" && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="pointer-events-auto">
            <form onSubmit={onSubmit} className="flex gap-2">
              <input
                ref={inputRef}
                type="text"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder="Type your message..."
                className="px-3 py-2 bg-white/10 backdrop-blur-md border border-white/20 rounded-lg text-white placeholder-white/50 focus:outline-none focus:border-white/40"
                autoFocus
              />
              <button
                type="submit"
                disabled={!inputValue.trim()}
                className="px-4 py-2 bg-blue-500/80 hover:bg-blue-500/60 rounded-lg text-white transition-colors disabled:opacity-50"
              >
                Send
              </button>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export function TransparentFloatingPanel({
  className,
  maxWidth = 400,
  opacity = 0.92,
  isWindowHovered = false,
  disableWindowManagement = false,
  onModeChange,
}: TransparentFloatingPanelProps) {
  // Use context hooks instead of local state
  const { voiceState } = useVoice();
  const agentState = useAgentState();
  const { recentMessages } = useRecentMessages();

  const [panelState, setPanelState] = useState<PanelState>({
    mode: "compact",
    agentStatus: "idle",
  });

  const [inputValue, setInputValue] = useState("");
  const [isHovered, setIsHovered] = useState(false);
  const [_isTransitioning, setIsTransitioning] = useState(false);
  const [isClickThroughEnabled, setIsClickThroughEnabled] = useState(true);

  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Window and panel management with delayed resizing for smooth transitions
  useEffect(() => {
    if (disableWindowManagement) {
      return;
    }

    const setupWindow = async () => {
      try {
        const appWindow = await Window.getByLabel("floating-panel");

        await appWindow?.setAlwaysOnTop(true);
        await appWindow?.setSkipTaskbar(true);
        await appWindow?.setResizable(false);

        setIsTransitioning(true);

        const resizeTimer = setTimeout(async () => {
          try {
            const dimensions = getWindowDimensions();
            await appWindow?.setSize(
              new LogicalSize(dimensions.width, dimensions.height)
            );

            setIsTransitioning(false);
          } catch (error) {
            console.error("Failed to resize window:", error);
            setIsTransitioning(false);
          }
        }, 750);

        return () => {
          clearTimeout(resizeTimer);
          setIsTransitioning(false);
        };
      } catch (error) {
        console.error("Failed to setup floating panel window:", error);
        setIsTransitioning(false);
      }
    };

    const cleanup = setupWindow();
    return () => {
      cleanup?.then((cleanupFn) => cleanupFn?.());
    };
  }, [panelState.mode, disableWindowManagement]);

  // Sync local panel state with context agent state
  useEffect(() => {
    setPanelState((prev) => ({
      ...prev,
      agentStatus: agentState.status,
      currentResponse: agentState.currentResponse,
      error: agentState.error,
    }));
  }, [agentState]);

  // Manage click-through behavior based on panel state and hover
  useEffect(() => {
    const shouldBeInteractive =
      isHovered ||
      isWindowHovered ||
      panelState.mode !== "compact" ||
      voiceState.isListening ||
      voiceState.isTranscribing ||
      voiceState.isSpeaking ||
      panelState.agentStatus !== "idle";

    const newClickThroughState = !shouldBeInteractive;

    if (newClickThroughState !== isClickThroughEnabled) {
      setIsClickThroughEnabled(newClickThroughState);

      invoke("set_floating_panel_click_through", {
        clickThrough: newClickThroughState,
      }).catch((error) => {
        console.warn("Failed to set click-through behavior:", error);
      });
    }
  }, [
    isHovered,
    isWindowHovered,
    panelState.mode,
    voiceState.isListening,
    voiceState.isTranscribing,
    voiceState.isSpeaking,
    panelState.agentStatus,
    isClickThroughEnabled,
  ]);

  // Auto-expand panel when there's activity or hover
  useEffect(() => {
    const hasActivity =
      voiceState.isListening ||
      voiceState.isTranscribing ||
      voiceState.isSpeaking ||
      panelState.agentStatus !== "idle";

    if ((hasActivity || isHovered) && panelState.mode === "compact") {
      setPanelState((prev) => ({ ...prev, mode: "expanded" }));
    } else if (!hasActivity && panelState.mode === "expanded" && !isHovered) {
      const timer = setTimeout(() => {
        setPanelState((prev) => ({ ...prev, mode: "compact" }));
      }, 2500);
      return () => clearTimeout(timer);
    }
  }, [
    voiceState.isListening,
    voiceState.isTranscribing,
    voiceState.isSpeaking,
    panelState.agentStatus,
    isHovered,
  ]);

  // Notify parent component when panel mode changes
  useEffect(() => {
    if (onModeChange) {
      onModeChange(panelState.mode);
    }
  }, [panelState.mode, onModeChange]);

  const getWindowDimensions = () => {
    switch (panelState.mode) {
      case "compact":
        return { width: 164, height: 84 };
      case "expanded":
        return { width: 324, height: 124 };
      case "chat":
        return { width: 374, height: 274 };
      case "settings":
        return { width: 304, height: 204 };
      default:
        return { width: 164, height: 84 };
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!inputValue.trim()) return;

    try {
      await invoke("submit_query", { query: inputValue });
      setInputValue("");
    } catch (error) {
      console.error("Failed to submit query:", error);
    }
  };

  return (
    <div
      ref={panelRef}
      className={cn(
        "relative rounded-xl border backdrop-blur-md transition-all duration-700 ease-out",
        "border-white/20 bg-gradient-to-br from-white/15 to-white/5",
        "shadow-2xl shadow-black/50",
        isClickThroughEnabled ? "pointer-events-none" : "pointer-events-auto",
        className
      )}
      style={{
        opacity,
        maxWidth,
        pointerEvents: isClickThroughEnabled ? "none" : "auto",
      }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      data-tauri-drag-region={panelState.mode === "compact"}
    >
      {/* Use VoiceAIBar for all modes */}
      <IntegratedVoiceAIBar
        panelState={panelState}
        voiceState={voiceState}
        recentMessages={recentMessages}
        onSubmit={handleSubmit}
        inputValue={inputValue}
        setInputValue={setInputValue}
        inputRef={inputRef}
      />

      {/* Traditional modes for chat and settings - overlay when needed */}
      {panelState.mode === "chat" && (
        <div className="absolute inset-0 bg-black/20 backdrop-blur-sm rounded-xl p-3 text-white flex flex-col">
          <div className="flex items-center justify-between mb-2" data-tauri-drag-region>
            <h3 className="text-sm font-medium">Recent Chat</h3>
            <div className="flex items-center gap-1">
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setPanelState((prev) => ({ ...prev, mode: "expanded" }));
                }}
                className="p-1 hover:bg-white/10 rounded transition-colors"
              >
                <ChevronUp className="h-3 w-3" />
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setPanelState((prev) => ({ ...prev, mode: "compact" }));
                }}
                className="p-1 hover:bg-white/10 rounded transition-colors"
              >
                <Minimize2 className="h-3 w-3" />
              </button>
            </div>
          </div>

          <div className="flex-1 overflow-y-auto space-y-2 mb-2">
            {recentMessages.length === 0 ? (
              <div className="text-xs text-white/50 text-center py-4">
                No recent messages
              </div>
            ) : (
              recentMessages.map((msg, idx) => (
                <div
                  key={idx}
                  className={cn(
                    "text-xs p-2 rounded",
                    msg.role === "user"
                      ? "bg-blue-500/70 text-blue-100"
                      : "bg-gray-500/70 text-gray-100"
                  )}
                >
                  {msg.content}
                </div>
              ))
            )}
          </div>

          <form onSubmit={handleSubmit} className="flex gap-1">
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder="Type a message..."
              className="flex-1 bg-black/70 border border-white/10 rounded px-2 py-1 text-xs text-white placeholder-white/50 focus:outline-none focus:border-white/80"
            />
            <button
              type="submit"
              disabled={!inputValue.trim()}
              className="px-2 py-1 bg-blue-500/80 hover:bg-blue-500/50 rounded text-xs transition-colors disabled:opacity-50"
            >
              Send
            </button>
          </form>
        </div>
      )}

      {panelState.mode === "settings" && (
        <div className="absolute inset-0 bg-black/20 backdrop-blur-sm rounded-xl p-3 text-white">
          <div className="flex items-center justify-between mb-3" data-tauri-drag-region>
            <h3 className="text-sm font-medium">Panel Settings</h3>
            <button
              onClick={(e) => {
                e.stopPropagation();
                setPanelState((prev) => ({ ...prev, mode: "expanded" }));
              }}
              className="p-1 hover:bg-white/10 rounded transition-colors"
            >
              <ChevronUp className="h-3 w-3" />
            </button>
          </div>

          <div className="space-y-3 text-xs">
            <div>
              <label className="block text-white/70 mb-1">Opacity</label>
              <input
                type="range"
                min="0.5"
                max="1"
                step="0.1"
                value={opacity}
                onChange={(e) => {
                  console.log("Opacity changed:", e.target.value);
                }}
                className="w-full"
              />
            </div>

            <div>
              <label className="block text-white/70 mb-1">Auto-hide</label>
              <label className="flex items-center gap-2">
                <input type="checkbox" className="rounded" />
                <span>Hide when inactive</span>
              </label>
            </div>

            <div>
              <label className="block text-white/70 mb-1">Voice Mode</label>
              <select className="w-full bg-black/70 border border-white/10 rounded px-2 py-1 text-white">
                <option>Always available</option>
                <option>Push-to-talk</option>
                <option>Disabled</option>
              </select>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default TransparentFloatingPanel;
