import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import {
  Brain,
  ChevronUp,
  Loader2,
  MessageSquare,
  Mic,
  Minimize2,
  Settings,
  Type,
  Volume2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";

// Enhanced state management for the transparent panel
interface PanelState {
  mode: "compact" | "expanded" | "chat" | "settings";
  agentStatus: "idle" | "listening" | "thinking" | "responding" | "error";
  voiceMode: "dictation" | "agent" | "idle";
  isListening: boolean;
  isTranscribing: boolean;
  isSpeaking: boolean;
  transcriptionText?: string;
  currentResponse?: string;
  error?: string;
  audioLevel: number;
}

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  isStreaming?: boolean;
}

interface TransparentFloatingPanelProps {
  className?: string;
  maxWidth?: number;
  opacity?: number;
}

export function TransparentFloatingPanel({
  className,
  maxWidth = 400,
  opacity = 0.92,
}: TransparentFloatingPanelProps) {
  // State management
  const [panelState, setPanelState] = useState<PanelState>({
    mode: "compact",
    agentStatus: "idle",
    voiceMode: "idle",
    isListening: false,
    isTranscribing: false,
    isSpeaking: false,
    audioLevel: 0,
  });

  const [recentMessages, setRecentMessages] = useState<ChatMessage[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [isHovered, setIsHovered] = useState(false);
  // Debug: Add some test messages for development
  useEffect(() => {
    if (recentMessages.length === 0) {
      setRecentMessages([
        {
          role: "user",
          content: "Hello Juno!",
          timestamp: Date.now() - 60000,
        },
        {
          role: "assistant",
          content:
            "Hi! I'm ready to help you. Try clicking the buttons or typing a message!",
          timestamp: Date.now() - 30000,
        },
      ]);
    }
  }, []);

  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Window and panel management
  useEffect(() => {
    const setupWindow = async () => {
      try {
        const appWindow = await Window.getByLabel("floating-panel");

        // Configure window based on panel mode
        const dimensions = getPanelDimensions();
        await appWindow?.setSize(
          new LogicalSize(dimensions.width, dimensions.height)
        );
        await appWindow?.setAlwaysOnTop(true);
        await appWindow?.setSkipTaskbar(true);
        await appWindow?.setResizable(false);
      } catch (error) {
        console.error("Failed to setup floating panel window:", error);
      }
    };

    setupWindow();
  }, [panelState.mode]);

  // Listen for agent events and update panel state
  useEffect(() => {
    const listeners: Array<() => void> = [];

    const setupListeners = async () => {
      // Agent status events
      listeners.push(
        await listen("agent-started", () => {
          setPanelState((prev) => ({ ...prev, agentStatus: "listening" }));
        })
      );

      listeners.push(
        await listen("agent-thinking", () => {
          setPanelState((prev) => ({ ...prev, agentStatus: "thinking" }));
        })
      );

      listeners.push(
        await listen("agent-responding", () => {
          setPanelState((prev) => ({ ...prev, agentStatus: "responding" }));
        })
      );

      // Voice events
      listeners.push(
        await listen("dictation-active", (event) => {
          const isActive = event.payload as boolean;
          setPanelState((prev) => ({
            ...prev,
            voiceMode: isActive ? "dictation" : "idle",
            isListening: isActive,
          }));
        })
      );

      listeners.push(
        await listen("app-dictation-started", () => {
          setPanelState((prev) => ({
            ...prev,
            voiceMode: "agent",
            isListening: true,
          }));
        })
      );

      listeners.push(
        await listen("app-dictation-finished", () => {
          setPanelState((prev) => ({
            ...prev,
            isListening: false,
          }));
        })
      );

      // Transcription events
      listeners.push(
        await listen("dictation-transcription-partial", (event) => {
          const text = event.payload as string;
          setPanelState((prev) => ({
            ...prev,
            isTranscribing: true,
            transcriptionText: text,
          }));
        })
      );

      listeners.push(
        await listen("dictation-transcription-final", (event) => {
          const text = event.payload as string;
          setPanelState((prev) => ({
            ...prev,
            isTranscribing: false,
            transcriptionText: text,
          }));

          // Add user message to recent messages
          setRecentMessages((prev) => [
            ...prev.slice(-4), // Keep last 4 messages
            {
              role: "user",
              content: text,
              timestamp: Date.now(),
            },
          ]);
        })
      );

      // AI response streaming
      listeners.push(
        await listen("streaming-text", (event) => {
          const chunk = event.payload as { chunk: string; message_id: string };
          setPanelState((prev) => ({
            ...prev,
            currentResponse: (prev.currentResponse || "") + chunk.chunk,
          }));
        })
      );

      listeners.push(
        await listen("stream-end", (event) => {
          const data = event.payload as {
            message_id: string;
            complete_text: string;
          };

          // Add assistant message to recent messages
          setRecentMessages((prev) => [
            ...prev.slice(-4),
            {
              role: "assistant",
              content: data.complete_text,
              timestamp: Date.now(),
            },
          ]);

          setPanelState((prev) => ({
            ...prev,
            currentResponse: undefined,
            agentStatus: "idle",
          }));
        })
      );

      // Audio level updates
      listeners.push(
        await listen<number>("audio-level", (event) => {
          setPanelState((prev) => ({ ...prev, audioLevel: event.payload }));
        })
      );

      // TTS events
      listeners.push(
        await listen("tts-started", () => {
          setPanelState((prev) => ({ ...prev, isSpeaking: true }));
        })
      );

      listeners.push(
        await listen("tts-finished", () => {
          setPanelState((prev) => ({ ...prev, isSpeaking: false }));
        })
      );
    };

    setupListeners();

    return () => {
      listeners.forEach((unlisten) => unlisten());
    };
  }, []);

  // Auto-expand panel when there's activity or hover
  useEffect(() => {
    const hasActivity =
      panelState.isListening ||
      panelState.isTranscribing ||
      panelState.isSpeaking ||
      panelState.agentStatus !== "idle";

    if ((hasActivity || isHovered) && panelState.mode === "compact") {
      setPanelState((prev) => ({ ...prev, mode: "expanded" }));
    } else if (!hasActivity && panelState.mode === "expanded" && !isHovered) {
      // Auto-collapse after a delay
      const timer = setTimeout(() => {
        setPanelState((prev) => ({ ...prev, mode: "compact" }));
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [
    panelState.isListening,
    panelState.isTranscribing,
    panelState.isSpeaking,
    panelState.agentStatus,
    isHovered,
  ]);

  // Note: Drag functionality now handled by Tauri's data-tauri-drag-region
  // This allows proper desktop window dragging instead of webview-only dragging

  // Get panel dimensions based on mode
  const getPanelDimensions = () => {
    switch (panelState.mode) {
      case "compact":
        return { width: 80, height: 40 };
      case "expanded":
        return { width: Math.min(maxWidth, 350), height: 120 };
      case "chat":
        return { width: Math.min(maxWidth, 400), height: 300 };
      case "settings":
        return { width: Math.min(maxWidth, 320), height: 200 };
      default:
        return { width: 80, height: 40 };
    }
  };

  // Get main status icon
  const getStatusIcon = () => {
    if (panelState.error) {
      return <X className="h-4 w-4 text-red-400" />;
    }

    if (panelState.isSpeaking) {
      return <Volume2 className="h-4 w-4 text-purple-400 animate-pulse" />;
    }

    if (panelState.agentStatus === "thinking") {
      return <Loader2 className="h-4 w-4 text-blue-400 animate-spin" />;
    }

    if (panelState.agentStatus === "responding") {
      return <Brain className="h-4 w-4 text-blue-400 animate-pulse" />;
    }

    if (panelState.isListening) {
      return panelState.voiceMode === "dictation" ? (
        <Type className="h-4 w-4 text-orange-400" />
      ) : (
        <Mic className="h-4 w-4 text-green-400" />
      );
    }

    if (panelState.isTranscribing) {
      return <Loader2 className="h-4 w-4 text-orange-400 animate-spin" />;
    }

    return <Brain className="h-4 w-4 text-gray-400" />;
  };

  // Get background color based on state
  const getBackgroundColor = () => {
    if (panelState.voiceMode === "dictation") {
      return "bg-gradient-to-br from-orange-500/20 to-orange-600/30";
    }
    if (panelState.voiceMode === "agent" || panelState.agentStatus !== "idle") {
      return "bg-gradient-to-br from-blue-500/20 to-blue-600/30";
    }
    if (panelState.isSpeaking) {
      return "bg-gradient-to-br from-purple-500/20 to-purple-600/30";
    }
    return "bg-gradient-to-br from-gray-800/20 to-gray-900/30";
  };

  // Audio level visualization
  const AudioLevelIndicator = () => {
    if (!panelState.isListening) return null;

    return (
      <div className="flex items-center gap-1">
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            className={cn(
              "w-1 rounded-full transition-all duration-150 audio-bar",
              panelState.audioLevel > (i + 1) * 20
                ? "bg-white/80 h-3"
                : "bg-white/20 h-1"
            )}
          />
        ))}
      </div>
    );
  };

  // Handle form submission
  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!inputValue.trim()) return;

    try {
      await invoke("submit_query", { query: inputValue });
      setInputValue("");
      setPanelState((prev) => ({ ...prev, mode: "expanded" }));
    } catch (error) {
      console.error("Failed to submit query:", error);
      setPanelState((prev) => ({ ...prev, error: "Failed to submit query" }));
    }
  };

  const dimensions = getPanelDimensions();

  return (
    <div
      ref={panelRef}
      data-tauri-drag-region
      className={cn(
        "w-screen h-screen bg-transparent overflow-hidden pointer-events-none select-none",
        className
      )}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div className="relative z-50 p-3" data-tauri-drag-region>
        <div
          data-tauri-drag-region
          className={cn(
            "rounded-2xl border border-white/10 backdrop-blur-xl glass-panel-dark",
            "panel-transition shadow-2xl shadow-black/20 pointer-events-auto",
            getBackgroundColor(),
            panelState.voiceMode === "dictation" && "panel-glow-orange",
            (panelState.voiceMode === "agent" ||
              panelState.agentStatus !== "idle") &&
              "panel-glow-blue",
            panelState.isSpeaking && "panel-glow-purple"
          )}
          style={{
            width: dimensions.width,
            height: dimensions.height,
            opacity,
          }}
        >
          {/* Compact Mode */}
          {panelState.mode === "compact" && (
            <div
              data-tauri-drag-region
              className="w-full h-full flex items-center justify-center p-2 cursor-pointer"
              onClick={(e) => {
                e.stopPropagation();
                setPanelState((prev) => ({ ...prev, mode: "expanded" }));
              }}
            >
              <div className="flex items-center gap-2">
                {getStatusIcon()}
                {panelState.audioLevel > 0 && <AudioLevelIndicator />}
              </div>
            </div>
          )}

          {/* Expanded Mode */}
          {panelState.mode === "expanded" && (
            <div className="w-full h-full p-3 text-white">
              {/* Header */}
              <div
                data-tauri-drag-region
                className="flex items-center justify-between mb-2"
              >
                <div className="flex items-center gap-2">
                  {getStatusIcon()}
                  <span className="text-xs font-medium">
                    {panelState.isSpeaking
                      ? "Speaking"
                      : panelState.agentStatus === "thinking"
                      ? "Thinking"
                      : panelState.agentStatus === "responding"
                      ? "Responding"
                      : panelState.isTranscribing
                      ? "Transcribing"
                      : panelState.isListening
                      ? panelState.voiceMode === "dictation"
                        ? "Dictating"
                        : "Listening"
                      : "Juno AI"}
                  </span>
                  <span className="text-xs text-white/50 ml-2">
                    ({panelState.mode})
                  </span>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setPanelState((prev) => ({ ...prev, mode: "chat" }));
                    }}
                    className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                    title="Open Chat"
                  >
                    <MessageSquare className="h-3 w-3" />
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setPanelState((prev) => ({ ...prev, mode: "settings" }));
                    }}
                    className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                    title="Settings"
                  >
                    <Settings className="h-3 w-3" />
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setPanelState((prev) => ({ ...prev, mode: "compact" }));
                    }}
                    className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                    title="Minimize"
                  >
                    <Minimize2 className="h-3 w-3" />
                  </button>
                </div>
              </div>

              {/* Content Area */}
              <div className="space-y-2">
                {/* Current Activity */}
                {(panelState.transcriptionText ||
                  panelState.currentResponse) && (
                  <div className="bg-black/20 rounded-lg p-2 text-xs">
                    {panelState.transcriptionText && (
                      <div className="text-orange-200">
                        "{panelState.transcriptionText}"
                      </div>
                    )}
                    {panelState.currentResponse && (
                      <div className="text-blue-200">
                        {panelState.currentResponse}
                      </div>
                    )}
                  </div>
                )}

                {/* Audio Level Indicator */}
                {panelState.isListening && (
                  <div className="flex items-center justify-center">
                    <AudioLevelIndicator />
                  </div>
                )}

                {/* Quick Input */}
                <form onSubmit={handleSubmit} className="flex gap-1">
                  <input
                    ref={inputRef}
                    type="text"
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onMouseDown={(e) => e.stopPropagation()}
                    onFocus={(e) => e.stopPropagation()}
                    placeholder="Quick ask..."
                    className="flex-1 bg-black/20 border border-white/10 rounded px-2 py-1 text-xs text-white placeholder-white/50 focus:outline-none focus:border-white/30 cursor-text"
                  />
                  <button
                    type="submit"
                    disabled={!inputValue.trim()}
                    onClick={(e) => e.stopPropagation()}
                    className="px-2 py-1 bg-blue-500/30 hover:bg-blue-500/50 rounded text-xs transition-colors disabled:opacity-50 cursor-pointer"
                  >
                    Ask
                  </button>
                </form>
              </div>
            </div>
          )}

          {/* Chat Mode */}
          {panelState.mode === "chat" && (
            <div className="w-full h-full p-3 text-white flex flex-col">
              {/* Header */}
              <div
                data-tauri-drag-region
                className="flex items-center justify-between mb-2"
              >
                <h3 className="text-sm font-medium">Recent Chat</h3>
                <div className="flex items-center gap-1">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setPanelState((prev) => ({ ...prev, mode: "expanded" }));
                    }}
                    className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                    title="Back"
                  >
                    <ChevronUp className="h-3 w-3" />
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setPanelState((prev) => ({ ...prev, mode: "compact" }));
                    }}
                    className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                    title="Minimize"
                  >
                    <Minimize2 className="h-3 w-3" />
                  </button>
                </div>
              </div>

              {/* Messages */}
              <div className="flex-1 overflow-y-auto space-y-2 mb-2 custom-scrollbar">
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
                          ? "bg-blue-500/20 text-blue-100"
                          : "bg-gray-500/20 text-gray-100"
                      )}
                    >
                      {msg.content}
                    </div>
                  ))
                )}
              </div>

              {/* Chat Input */}
              <form onSubmit={handleSubmit} className="flex gap-1">
                <input
                  type="text"
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onMouseDown={(e) => e.stopPropagation()}
                  onFocus={(e) => e.stopPropagation()}
                  placeholder="Type a message..."
                  className="flex-1 bg-black/20 border border-white/10 rounded px-2 py-1 text-xs text-white placeholder-white/50 focus:outline-none focus:border-white/30 cursor-text"
                />
                <button
                  type="submit"
                  disabled={!inputValue.trim()}
                  onClick={(e) => e.stopPropagation()}
                  className="px-2 py-1 bg-blue-500/30 hover:bg-blue-500/50 rounded text-xs transition-colors disabled:opacity-50 cursor-pointer"
                >
                  Send
                </button>
              </form>
            </div>
          )}

          {/* Settings Mode */}
          {panelState.mode === "settings" && (
            <div className="w-full h-full p-3 text-white">
              {/* Header */}
              <div
                data-tauri-drag-region
                className="flex items-center justify-between mb-3"
              >
                <h3 className="text-sm font-medium">Panel Settings</h3>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setPanelState((prev) => ({ ...prev, mode: "expanded" }));
                  }}
                  className="p-1 hover:bg-white/10 rounded transition-colors cursor-pointer"
                  title="Back"
                >
                  <ChevronUp className="h-3 w-3" />
                </button>
              </div>

              {/* Settings Content */}
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
                      // This would need to be handled by parent component
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
                  <select className="w-full bg-black/20 border border-white/10 rounded px-2 py-1 text-white">
                    <option>Always available</option>
                    <option>Push-to-talk</option>
                    <option>Disabled</option>
                  </select>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default TransparentFloatingPanel;
