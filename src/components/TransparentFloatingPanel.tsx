import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Window, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";

// Enhanced state management for production-ready computer use panel
interface PanelState {
  mode:
    | "compact"
    | "expanded"
    | "chat"
    | "settings"
    | "computer-use"
    | "help"
    | "pill";
  agentStatus:
    | "idle"
    | "listening"
    | "thinking"
    | "responding"
    | "error"
    | "computer-use-active"
    | "paused";
  voiceMode:
    | "dictation"
    | "agent"
    | "idle"
    | "listening"
    | "transcribing"
    | "completed";
  computerUseMode:
    | "idle"
    | "screenshot"
    | "click"
    | "type"
    | "scroll"
    | "key"
    | "waiting"
    | "drag"
    | "double-click"
    | "active"
    | "executing"
    | "completed";
  isListening: boolean;
  isTranscribing: boolean;
  isSpeaking: boolean;
  isComputerUseActive: boolean;
  isPaused: boolean;
  transcriptionText?: string;
  currentResponse?: string;
  currentTool?: string;
  error?: string;
  audioLevel: number;
  confidence?: number; // Voice confidence score
  permissionsGranted: boolean;
  lastScreenshot?: string;
  connectionStatus:
    | "connected"
    | "disconnected"
    | "connecting"
    | "reconnecting"
    | "error";
  performance: {
    responseTime: number;
    successRate: number;
    lastCommandTime?: number;
  };
}

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  isStreaming?: boolean;
  toolCalls?: ToolCall[];
  error?: string;
  confidence?: number;
}

interface ToolCall {
  id: string;
  name: string;
  status: "running" | "completed" | "failed" | "cancelled";
  description?: string;
  startTime: number;
  endTime?: number;
  error?: string;
  result?: any;
}

interface TransparentFloatingPanelProps {
  isWindowHovered?: boolean;
}

// Production-ready computer use panel with enhanced capabilities
export default function TransparentFloatingPanel({
  isWindowHovered = false,
}: TransparentFloatingPanelProps) {
  // Enhanced state management
  const [panelState, setPanelState] = useState<PanelState>({
    mode: "compact",
    agentStatus: "idle",
    voiceMode: "idle",
    computerUseMode: "idle",
    isListening: false,
    isTranscribing: false,
    isSpeaking: false,
    isComputerUseActive: false,
    isPaused: false,
    audioLevel: 0,
    permissionsGranted: false,
    connectionStatus: "disconnected",
    performance: {
      responseTime: 0,
      successRate: 100,
    },
  });

  const [recentMessages, setRecentMessages] = useState<ChatMessage[]>([]);
  const [isHovered, setIsHovered] = useState(false);
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [isClickThroughEnabled, setIsClickThroughEnabled] = useState(true);
  const [activeTool, setActiveTool] = useState<ToolCall | null>(null);
  const [retryCount, setRetryCount] = useState(0);
  const [voiceSettings, setVoiceSettings] = useState({
    enabled: true,
    wakeWordEnabled: false,
    confidenceThreshold: 0.7,
    autoSubmit: true,
    sensitivity: 0.5,
    noiseFiltering: true,
    volume: 0.8,
  });

  const errorTimeoutRef = useRef<NodeJS.Timeout>();
  const retryTimeoutRef = useRef<NodeJS.Timeout>();
  const performanceTimerRef = useRef<NodeJS.Timeout>();
  const lastActivityRef = useRef<number>(Date.now());

  // Utility functions
  const getPanelDimensions = useCallback(() => {
    switch (panelState.mode) {
      case "compact":
        return { width: 120, height: 40 };
      case "expanded":
        return { width: 320, height: 200 };
      case "chat":
        return { width: 380, height: 300 };
      case "settings":
        return { width: 320, height: 250 };
      case "pill":
        return { width: 120, height: 40 };
      default:
        return { width: 320, height: 200 };
    }
  }, [panelState.mode]);

  const getWindowDimensions = useCallback(() => {
    const base = getPanelDimensions();
    return {
      width: Math.max(base.width, 120),
      height: Math.max(base.height, 40),
    };
  }, [getPanelDimensions]);

  const getComputerUseMode = useCallback((toolName: string) => {
    if (toolName.includes("screenshot")) return "screenshot";
    if (toolName.includes("click")) return "click";
    if (toolName.includes("type")) return "type";
    if (toolName.includes("scroll")) return "scroll";
    if (toolName.includes("key")) return "key";
    if (toolName.includes("drag")) return "drag";
    if (toolName.includes("double")) return "double-click";
    return "waiting";
  }, []);

  // Vibrancy control functions
  const applyVibrancy = useCallback(async () => {
    try {
      await invoke("apply_pill_vibrancy", { windowLabel: "floating-panel" });
      console.log("Applied pill vibrancy to floating panel");
    } catch (error) {
      console.error("Failed to apply vibrancy:", error);
    }
  }, []);

  const removeVibrancy = useCallback(async () => {
    try {
      await invoke("remove_vibrancy", { windowLabel: "floating-panel" });
      console.log("Removed vibrancy from floating panel");
    } catch (error) {
      console.error("Failed to remove vibrancy:", error);
    }
  }, []);

  const createPillWindow = useCallback(
    async (width: number = 200, height: number = 50) => {
      try {
        await invoke("create_pill_window", {
          label: "pill-window",
          width,
          height,
          x: 100,
          y: 100,
        });
        console.log("Created new pill window with vibrancy");
      } catch (error) {
        console.error("Failed to create pill window:", error);
      }
    },
    []
  );

  // Enhanced error handling with automatic recovery
  const handleError = useCallback((errorMessage: string) => {
    console.error("Panel error:", errorMessage);
    setPanelState((prev) => ({
      ...prev,
      error: errorMessage,
      agentStatus: "error",
    }));

    // Clear previous error timeout
    if (errorTimeoutRef.current) {
      clearTimeout(errorTimeoutRef.current);
    }

    // Auto-recovery mechanism
    errorTimeoutRef.current = setTimeout(() => {
      setPanelState((prev) => ({
        ...prev,
        error: undefined,
        agentStatus: "idle",
      }));
    }, 5000);
  }, []);

  const handleQuickAction = useCallback(
    async (action: string) => {
      try {
        switch (action) {
          case "screenshot":
            await invoke("take_screenshot");
            break;
          case "emergency-stop":
            await invoke("emergency_stop_agent");
            break;
          case "toggle-voice":
            await invoke("toggle_voice_mode");
            break;
          default:
            console.warn("Unknown quick action:", action);
        }
      } catch (error) {
        console.error("Quick action failed:", error);
        handleError(`Quick action failed: ${error}`);
      }
    },
    [handleError]
  );

  // Performance optimization - memoized calculations

  // Initialize panel and check permissions
  useEffect(() => {
    const initializePanel = async () => {
      try {
        // Check permissions
        const permissionsStatus = await invoke(
          "check_accessibility_permissions"
        );
        setPanelState((prev) => ({
          ...prev,
          permissionsGranted: permissionsStatus as boolean,
          connectionStatus: "connected",
        }));

        // Load recent messages from storage
        try {
          const savedMessages = await invoke("get_recent_chat_messages", {
            limit: 10,
          });
          if (savedMessages) {
            setRecentMessages(savedMessages as ChatMessage[]);
          }
        } catch (error) {
          console.warn("Failed to load recent messages:", error);
        }

        // Load voice settings
        try {
          const settings = await invoke("get_voice_settings");
          if (settings) {
            setVoiceSettings(settings as typeof voiceSettings);
          }
        } catch (error) {
          console.warn("Failed to load voice settings:", error);
        }

        // Initialize performance monitoring
        startPerformanceMonitoring();
      } catch (error) {
        console.error("Failed to initialize panel:", error);
        setPanelState((prev) => ({
          ...prev,
          connectionStatus: "disconnected",
          error: "Failed to initialize panel",
        }));
      }
    };

    initializePanel();
  }, []);

  // Performance monitoring
  const startPerformanceMonitoring = useCallback(() => {
    performanceTimerRef.current = setInterval(() => {
      const now = Date.now();
      const timeSinceLastActivity = now - lastActivityRef.current;

      // Update performance metrics
      setPanelState((prev) => ({
        ...prev,
        performance: {
          ...prev.performance,
          lastCommandTime: timeSinceLastActivity,
        },
      }));
    }, 1000);

    return () => {
      if (performanceTimerRef.current) {
        clearInterval(performanceTimerRef.current);
      }
    };
  }, []);

  // Enhanced keyboard shortcuts handler
  useEffect(() => {
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      // Only handle shortcuts when panel is focused or hovered
      if (!isHovered && panelState.mode === "compact") return;

      const { key, metaKey, shiftKey } = event;

      // Prevent default for our shortcuts
      if (
        (metaKey && shiftKey && key === "J") ||
        (metaKey && shiftKey && key === "S") ||
        key === "Escape" ||
        (metaKey && key === "m")
      ) {
        event.preventDefault();
        event.stopPropagation();
      }

      // Handle shortcuts
      if (metaKey && shiftKey && key === "J") {
        // Toggle panel mode
        setPanelState((prev) => ({
          ...prev,
          mode: prev.mode === "compact" ? "expanded" : "compact",
        }));
      } else if (metaKey && shiftKey && key === "S") {
        // Quick screenshot
        handleQuickAction("screenshot");
      } else if (key === "Escape") {
        // Emergency stop
        if (panelState.isComputerUseActive) {
          handleQuickAction("emergency-stop");
        } else if (panelState.mode !== "compact") {
          setPanelState((prev) => ({ ...prev, mode: "compact" }));
        }
      } else if (metaKey && key === "m") {
        // Toggle voice
        handleQuickAction("toggle-voice");
      } else if (key === " " && panelState.mode !== "compact") {
        // Spacebar for quick pause/resume
        event.preventDefault();
        togglePauseResume();
      }

      // Mode-specific shortcuts
      if (panelState.mode === "expanded") {
        if (key === "1") setPanelState((prev) => ({ ...prev, mode: "chat" }));
        if (key === "2")
          setPanelState((prev) => ({ ...prev, mode: "computer-use" }));
        if (key === "3")
          setPanelState((prev) => ({ ...prev, mode: "settings" }));
        if (key === "4") setPanelState((prev) => ({ ...prev, mode: "help" }));
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isHovered, panelState.mode, panelState.isComputerUseActive]);

  // Pause/Resume functionality
  const togglePauseResume = useCallback(async () => {
    try {
      if (panelState.isPaused) {
        await invoke("resume_agent_operations");
        setPanelState((prev) => ({
          ...prev,
          isPaused: false,
          agentStatus: "idle",
        }));
      } else {
        await invoke("pause_agent_operations");
        setPanelState((prev) => ({
          ...prev,
          isPaused: true,
          agentStatus: "paused",
        }));
      }
    } catch (error) {
      handleError(
        `Failed to ${panelState.isPaused ? "resume" : "pause"}: ${error}`
      );
    }
  }, [panelState.isPaused]);

  // Enhanced activity tracking
  const trackActivity = useCallback(() => {
    lastActivityRef.current = Date.now();
    setPanelState((prev) => ({
      ...prev,
      performance: {
        ...prev.performance,
        lastCommandTime: Date.now(),
      },
    }));
  }, []);

  // Window and panel management with enhanced transitions
  useEffect(() => {
    const setupWindow = async () => {
      try {
        const appWindow = await Window.getByLabel("floating-panel");

        // Configure enhanced window properties
        await appWindow?.setAlwaysOnTop(true);
        await appWindow?.setSkipTaskbar(true);
        await appWindow?.setResizable(false);

        setIsTransitioning(true);

        // Enhanced window resize with animation support
        const resizeTimer = setTimeout(async () => {
          try {
            const dimensions = getWindowDimensions();
            await appWindow?.setSize(
              new LogicalSize(dimensions.width, dimensions.height)
            );
            setIsTransitioning(false);
          } catch (error) {
            console.error("Failed to resize window:", error);
          }
        }, 800); // Longer delay for complex animations

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
  }, [panelState.mode]);

  // Enhanced event listeners with error handling and retry logic
  useEffect(() => {
    const listeners: Array<() => void> = [];
    let reconnectAttempts = 0;
    const maxReconnectAttempts = 5;

    const setupListeners = async () => {
      try {
        // Agent status events with enhanced error handling
        listeners.push(
          await listen("agent-started", () => {
            setPanelState((prev) => ({
              ...prev,
              agentStatus: "listening",
              error: undefined,
              connectionStatus: "connected",
            }));
            setRetryCount(0);
            trackActivity();
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

        listeners.push(
          await listen("agent-finished", () => {
            setPanelState((prev) => ({ ...prev, agentStatus: "idle" }));
          })
        );

        listeners.push(
          await listen("agent-error", (event) => {
            const error = event.payload as string;
            handleError(`Agent error: ${error}`);
          })
        );

        // Computer use tool events with enhanced tracking
        listeners.push(
          await listen("computer-use-started", (event) => {
            const data = event.payload as {
              tool: string;
              description: string;
            };

            const newToolCall: ToolCall = {
              id: Date.now().toString(),
              name: data.tool,
              description: data.description,
              status: "running",
              startTime: Date.now(),
            };

            setActiveTool(newToolCall);
            setPanelState((prev) => ({
              ...prev,
              isComputerUseActive: true,
              agentStatus: "computer-use-active",
              computerUseMode: getComputerUseMode(data.tool),
              currentTool: data.tool,
            }));
            trackActivity();
          })
        );

        listeners.push(
          await listen("computer-use-completed", (event) => {
            const data = event.payload as {
              tool: string;
              result?: any;
            };

            if (activeTool) {
              const completedTool = {
                ...activeTool,
                status: "completed" as const,
                endTime: Date.now(),
                result: data.result,
              };

              setActiveTool(completedTool);

              // Update recent messages with tool call
              setRecentMessages((prev) => {
                const lastMessage = prev[prev.length - 1];
                if (lastMessage && lastMessage.role === "assistant") {
                  return [
                    ...prev.slice(0, -1),
                    {
                      ...lastMessage,
                      toolCalls: [
                        ...(lastMessage.toolCalls || []),
                        completedTool,
                      ],
                    },
                  ];
                }
                return prev;
              });
            }

            setPanelState((prev) => ({
              ...prev,
              isComputerUseActive: false,
              agentStatus: "idle",
              computerUseMode: "idle",
              currentTool: undefined,
            }));
          })
        );

        // Enhanced voice events
        listeners.push(
          await listen("voice-started", (event) => {
            const data = event.payload as { mode: "dictation" | "agent" };
            setPanelState((prev) => ({
              ...prev,
              isListening: true,
              voiceMode: data.mode,
              agentStatus: "listening",
            }));
          })
        );

        listeners.push(
          await listen("voice-stopped", () => {
            setPanelState((prev) => ({
              ...prev,
              isListening: false,
              voiceMode: "idle",
              agentStatus: "idle",
              audioLevel: 0,
            }));
          })
        );

        listeners.push(
          await listen("transcription-partial", (event) => {
            const data = event.payload as {
              text: string;
              confidence?: number;
            };
            setPanelState((prev) => ({
              ...prev,
              transcriptionText: data.text,
              confidence: data.confidence,
              isTranscribing: true,
            }));
          })
        );

        listeners.push(
          await listen("transcription-final", (event) => {
            const data = event.payload as {
              text: string;
              confidence?: number;
            };
            setPanelState((prev) => ({
              ...prev,
              transcriptionText: data.text,
              confidence: data.confidence,
              isTranscribing: false,
            }));

            // Auto-submit if confidence is high enough and enabled
            if (
              voiceSettings.autoSubmit &&
              data.confidence &&
              data.confidence >= voiceSettings.confidenceThreshold
            ) {
              handleVoiceQuery(data.text);
            }
          })
        );

        // Enhanced AI response streaming
        listeners.push(
          await listen("streaming-text", (event) => {
            const chunk = event.payload as {
              chunk: string;
              message_id: string;
            };
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
              tool_calls?: ToolCall[];
            };

            // Enhanced message with tool call information
            const newMessage: ChatMessage = {
              id: Date.now().toString(),
              role: "assistant",
              content: data.complete_text,
              timestamp: Date.now(),
              toolCalls: data.tool_calls,
            };

            setRecentMessages((prev) => [...prev.slice(-9), newMessage]);

            setPanelState((prev) => ({
              ...prev,
              currentResponse: undefined,
              agentStatus: "idle",
            }));
          })
        );

        // Audio level updates with enhanced visualization
        listeners.push(
          await listen<number>("audio-level", (event) => {
            setPanelState((prev) => ({ ...prev, audioLevel: event.payload }));
          })
        );

        // Enhanced TTS events
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

        // Permission change events
        listeners.push(
          await listen("permissions-changed", (event) => {
            const granted = event.payload as boolean;
            setPanelState((prev) => ({
              ...prev,
              permissionsGranted: granted,
            }));
          })
        );

        // Error events with retry logic
        listeners.push(
          await listen("system-error", (event) => {
            const error = event.payload as string;
            handleError(error);
          })
        );

        setPanelState((prev) => ({
          ...prev,
          connectionStatus: "connected",
        }));
      } catch (error) {
        console.error("Failed to setup event listeners:", error);

        // Attempt reconnection with exponential backoff
        if (reconnectAttempts < maxReconnectAttempts) {
          reconnectAttempts++;
          const delay = Math.pow(2, reconnectAttempts) * 1000;

          setTimeout(() => {
            console.log(
              `Attempting to reconnect (${reconnectAttempts}/${maxReconnectAttempts})`
            );
            setupListeners();
          }, delay);
        } else {
          setPanelState((prev) => ({
            ...prev,
            connectionStatus: "disconnected",
            error: "Failed to connect to agent system",
          }));
        }
      }
    };

    setupListeners();

    return () => {
      listeners.forEach((unlisten) => unlisten());
    };
  }, [voiceSettings, activeTool, handleError]);

  // Enhanced click-through behavior management
  useEffect(() => {
    const shouldBeInteractive =
      isHovered ||
      isWindowHovered ||
      panelState.mode !== "compact" ||
      panelState.isListening ||
      panelState.isTranscribing ||
      panelState.isSpeaking ||
      panelState.agentStatus !== "idle" ||
      panelState.isComputerUseActive;

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
    panelState.isListening,
    panelState.isTranscribing,
    panelState.isSpeaking,
    panelState.agentStatus,
    panelState.isComputerUseActive,
    isClickThroughEnabled,
  ]);

  // Enhanced auto-expand logic with computer use integration
  useEffect(() => {
    const hasActivity =
      panelState.isListening ||
      panelState.isTranscribing ||
      panelState.isSpeaking ||
      panelState.agentStatus !== "idle" ||
      panelState.isComputerUseActive;

    if ((hasActivity || isHovered) && panelState.mode === "compact") {
      setPanelState((prev) => ({ ...prev, mode: "expanded" }));
    } else if (!hasActivity && panelState.mode === "expanded" && !isHovered) {
      const timer = setTimeout(() => {
        setPanelState((prev) => ({ ...prev, mode: "compact" }));
      }, 3000); // Increased delay for computer use operations
      return () => clearTimeout(timer);
    }
  }, [
    panelState.isListening,
    panelState.isTranscribing,
    panelState.isSpeaking,
    panelState.agentStatus,
    panelState.isComputerUseActive,
    isHovered,
  ]);

  // Connection recovery with exponential backoff
  const attemptReconnection = useCallback(async () => {
    if (retryCount >= 3) {
      handleError("Max retry attempts reached");
      return;
    }

    setPanelState((prev) => ({
      ...prev,
      connectionStatus: "reconnecting",
    }));

    try {
      // Test connection with backend
      await invoke("get_floating_panel_state");
      setPanelState((prev) => ({
        ...prev,
        connectionStatus: "connected",
        error: undefined,
      }));
      setRetryCount(0);
    } catch (error) {
      const delay = Math.pow(2, retryCount) * 1000; // Exponential backoff
      setRetryCount((prev) => prev + 1);

      retryTimeoutRef.current = setTimeout(() => {
        attemptReconnection();
      }, delay);
    }
  }, [retryCount, handleError]);

  // Voice query handler with enhanced error handling
  const handleVoiceQuery = useCallback(
    async (query: string) => {
      try {
        await invoke("submit_query", { query });
        setPanelState((prev) => ({ ...prev, mode: "expanded" }));
      } catch (error) {
        console.error("Failed to submit voice query:", error);
        handleError(`Failed to process voice command: ${error}`);
      }
    },
    [handleError]
  );

  // Manual panel controls
  const handleToggleMode = useCallback(() => {
    setPanelState((prev) => ({
      ...prev,
      mode: prev.mode === "expanded" ? "compact" : "expanded",
    }));
  }, []);

  const handleTogglePillMode = useCallback(() => {
    setPanelState((prev) => ({
      ...prev,
      mode: prev.mode === "pill" ? "expanded" : "pill",
    }));

    // Apply vibrancy when switching to pill mode
    if (panelState.mode !== "pill") {
      setTimeout(applyVibrancy, 100);
    }
  }, [panelState.mode, applyVibrancy]);

  const handleShowSettings = useCallback(() => {
    setPanelState((prev) => ({ ...prev, mode: "settings" }));
  }, []);

  const handleShowChat = useCallback(() => {
    setPanelState((prev) => ({ ...prev, mode: "chat" }));
  }, []);

  const handlePauseResume = useCallback(async () => {
    if (panelState.isPaused) {
      setPanelState((prev) => ({ ...prev, isPaused: false }));
      await invoke("resume_agent").catch(console.error);
    } else {
      setPanelState((prev) => ({ ...prev, isPaused: true }));
      await invoke("pause_agent").catch(console.error);
    }
  }, [panelState.isPaused]);

  // Enhanced render functions for different UI elements
  const renderConnectionStatus = useMemo(() => {
    const statusConfig = {
      connected: { color: "text-green-400", label: "Connected", icon: "●" },
      disconnected: { color: "text-red-400", label: "Disconnected", icon: "●" },
      connecting: { color: "text-blue-400", label: "Connecting...", icon: "●" },
      reconnecting: {
        color: "text-yellow-400",
        label: "Reconnecting...",
        icon: "●",
      },
    };

    const config = statusConfig[panelState.connectionStatus];

    return (
      <div className={`flex items-center space-x-1 text-xs ${config.color}`}>
        <span className="animate-pulse">{config.icon}</span>
        <span>{config.label}</span>
      </div>
    );
  }, [panelState.connectionStatus]);

  const renderAgentStatus = useMemo(() => {
    const statusConfig = {
      idle: { color: "text-gray-400", label: "Ready", icon: "⚪" },
      listening: { color: "text-blue-400", label: "Listening", icon: "🎤" },
      thinking: { color: "text-yellow-400", label: "Thinking", icon: "🧠" },
      responding: { color: "text-green-400", label: "Responding", icon: "💬" },
      "computer-use-active": {
        color: "text-purple-400",
        label: "Computer Use",
        icon: "🖱️",
      },
      error: { color: "text-red-400", label: "Error", icon: "❌" },
      paused: { color: "text-orange-400", label: "Paused", icon: "⏸️" },
    };

    const config = statusConfig[panelState.agentStatus] || statusConfig.idle;

    return (
      <div className={`flex items-center space-x-1 text-xs ${config.color}`}>
        <span
          className={panelState.agentStatus !== "idle" ? "animate-pulse" : ""}
        >
          {config.icon}
        </span>
        <span>{config.label}</span>
        {panelState.currentTool && (
          <span className="text-xs text-gray-300">
            ({panelState.currentTool})
          </span>
        )}
      </div>
    );
  }, [panelState.agentStatus, panelState.currentTool]);

  const renderAudioLevel = useMemo(() => {
    if (!panelState.isListening && !panelState.audioLevel) return null;

    const level = Math.min(100, Math.max(0, panelState.audioLevel * 100));

    return (
      <div className="flex items-center space-x-2">
        <div className="w-16 h-1 bg-gray-600 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-green-400 to-yellow-400 transition-all duration-75"
            style={{ width: `${level}%` }}
          />
        </div>
        <span className="text-xs text-gray-300">{Math.round(level)}%</span>
      </div>
    );
  }, [panelState.isListening, panelState.audioLevel]);

  const renderTranscription = useMemo(() => {
    if (!panelState.transcriptionText && !panelState.isTranscribing)
      return null;

    return (
      <div className="bg-gray-800/60 rounded-lg p-2 border border-gray-600">
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs text-blue-400">
            {panelState.isTranscribing
              ? "Transcribing..."
              : "Final Transcription"}
          </span>
          {panelState.confidence && (
            <span
              className={`text-xs ${
                panelState.confidence >= 0.8
                  ? "text-green-400"
                  : panelState.confidence >= 0.6
                  ? "text-yellow-400"
                  : "text-red-400"
              }`}
            >
              {Math.round(panelState.confidence * 100)}%
            </span>
          )}
        </div>
        <p className="text-sm text-white">
          {panelState.transcriptionText || (
            <span className="text-gray-400 italic">Processing audio...</span>
          )}
        </p>
      </div>
    );
  }, [
    panelState.transcriptionText,
    panelState.isTranscribing,
    panelState.confidence,
  ]);

  const renderActiveTool = useMemo(() => {
    if (!activeTool) return null;

    const duration = activeTool.endTime
      ? activeTool.endTime - activeTool.startTime
      : Date.now() - activeTool.startTime;

    return (
      <div className="bg-purple-900/40 rounded-lg p-2 border border-purple-500/30">
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs text-purple-400">Active Tool</span>
          <span className="text-xs text-gray-300">
            {Math.round(duration / 1000)}s
          </span>
        </div>
        <div className="text-sm text-white font-medium">{activeTool.name}</div>
        <div className="text-xs text-gray-300">{activeTool.description}</div>
        <div
          className={`mt-1 text-xs font-medium ${
            activeTool.status === "running"
              ? "text-yellow-400"
              : activeTool.status === "completed"
              ? "text-green-400"
              : "text-red-400"
          }`}
        >
          {activeTool.status.charAt(0).toUpperCase() +
            activeTool.status.slice(1)}
        </div>
      </div>
    );
  }, [activeTool]);

  const renderRecentMessages = useMemo(() => {
    if (recentMessages.length === 0) return null;

    return (
      <div className="space-y-2 max-h-48 overflow-y-auto">
        {recentMessages.slice(-3).map((message) => (
          <div
            key={message.id}
            className={`rounded-lg p-2 text-xs ${
              message.role === "user"
                ? "bg-blue-900/40 border border-blue-500/30"
                : "bg-gray-800/60 border border-gray-600"
            }`}
          >
            <div className="flex items-center justify-between mb-1">
              <span
                className={`font-medium ${
                  message.role === "user" ? "text-blue-400" : "text-green-400"
                }`}
              >
                {message.role === "user" ? "You" : "Assistant"}
              </span>
              <span className="text-gray-400">
                {new Date(message.timestamp).toLocaleTimeString()}
              </span>
            </div>
            <p className="text-white">{message.content}</p>
            {message.toolCalls && message.toolCalls.length > 0 && (
              <div className="mt-1 space-y-1">
                {message.toolCalls.map((tool) => (
                  <div
                    key={tool.id}
                    className="bg-gray-700/50 rounded px-2 py-1"
                  >
                    <span className="text-purple-400 text-xs">{tool.name}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    );
  }, [recentMessages]);

  const renderError = useMemo(() => {
    if (!panelState.error) return null;

    return (
      <div className="bg-red-900/40 rounded-lg p-2 border border-red-500/30">
        <div className="flex items-center space-x-2">
          <span className="text-red-400">⚠️</span>
          <span className="text-xs text-red-400 font-medium">Error</span>
        </div>
        <p className="text-xs text-white mt-1">{panelState.error}</p>
        <button
          onClick={() =>
            setPanelState((prev) => ({ ...prev, error: undefined }))
          }
          className="mt-2 text-xs text-red-300 hover:text-red-100 transition-colors"
        >
          Dismiss
        </button>
      </div>
    );
  }, [panelState.error]);

  const renderCompactMode = useMemo(
    () => (
      <div
        className="flex items-center space-x-2 p-2"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {/* Status indicator */}
        <div
          className={`w-3 h-3 rounded-full ${
            panelState.agentStatus === "idle"
              ? "bg-gray-400"
              : panelState.agentStatus === "listening"
              ? "bg-blue-400 animate-pulse"
              : panelState.agentStatus === "thinking"
              ? "bg-yellow-400 animate-pulse"
              : panelState.agentStatus === "responding"
              ? "bg-green-400 animate-pulse"
              : panelState.agentStatus === "computer-use-active"
              ? "bg-purple-400 animate-pulse"
              : "bg-red-400 animate-pulse"
          }`}
        />

        {/* Voice mode indicator */}
        {panelState.voiceMode !== "idle" && (
          <span className="text-xs text-blue-400">
            {panelState.voiceMode === "dictation" ? "📝" : "🎤"}
          </span>
        )}

        {/* Computer use indicator */}
        {panelState.isComputerUseActive && (
          <span className="text-xs text-purple-400 animate-pulse">🖱️</span>
        )}

        {/* Audio level for compact mode */}
        {panelState.isListening && renderAudioLevel}

        {/* Pill mode toggle button */}
        <button
          onClick={handleTogglePillMode}
          className="p-1 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-xs"
          title="Toggle Pill Mode"
        >
          💊
        </button>
      </div>
    ),
    [panelState, renderAudioLevel, handleTogglePillMode]
  );

  const renderExpandedMode = useMemo(
    () => (
      <div
        className="p-4 space-y-3"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {/* Header with status indicators */}
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            {renderConnectionStatus}
            {renderAgentStatus}
          </div>
          <div className="flex items-center space-x-2">
            <button
              onClick={handlePauseResume}
              className={`p-1 rounded transition-colors ${
                panelState.isPaused
                  ? "text-green-400 hover:text-green-300"
                  : "text-yellow-400 hover:text-yellow-300"
              }`}
              title={panelState.isPaused ? "Resume Agent" : "Pause Agent"}
            >
              {panelState.isPaused ? "▶️" : "⏸️"}
            </button>
            <button
              onClick={handleShowSettings}
              className="text-gray-400 hover:text-gray-300 transition-colors"
              title="Settings"
            >
              ⚙️
            </button>
            <button
              onClick={handleShowChat}
              className="text-gray-400 hover:text-gray-300 transition-colors"
              title="Show Chat"
            >
              💬
            </button>
          </div>
        </div>

        {/* Audio level visualization */}
        {panelState.isListening && renderAudioLevel}

        {/* Transcription display */}
        {renderTranscription}

        {/* Active tool information */}
        {renderActiveTool}

        {/* Current AI response */}
        {panelState.currentResponse && (
          <div className="bg-gray-800/60 rounded-lg p-2 border border-gray-600">
            <div className="text-xs text-green-400 mb-1">AI Response</div>
            <p className="text-sm text-white">{panelState.currentResponse}</p>
          </div>
        )}

        {/* Error display */}
        {renderError}

        {/* Performance info in expanded mode */}
        {panelState.performance.lastCommandTime && (
          <div className="text-xs text-gray-400">
            Last activity:{" "}
            {Math.round(
              (Date.now() - panelState.performance.lastCommandTime) / 1000
            )}
            s ago
          </div>
        )}
      </div>
    ),
    [
      panelState,
      renderConnectionStatus,
      renderAgentStatus,
      renderAudioLevel,
      renderTranscription,
      renderActiveTool,
      renderError,
      handlePauseResume,
      handleShowSettings,
      handleShowChat,
    ]
  );

  const renderChatMode = useMemo(
    () => (
      <div
        className="p-4 space-y-3 max-h-96"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-600 pb-2">
          <h3 className="text-sm font-medium text-white">
            Recent Conversations
          </h3>
          <button
            onClick={handleToggleMode}
            className="text-gray-400 hover:text-gray-300 transition-colors"
            title="Back to Panel"
          >
            ←
          </button>
        </div>

        {/* Messages */}
        {renderRecentMessages}

        {/* No messages state */}
        {recentMessages.length === 0 && (
          <div className="text-center text-gray-400 text-sm py-4">
            No recent conversations
          </div>
        )}
      </div>
    ),
    [recentMessages, renderRecentMessages, handleToggleMode]
  );

  const renderSettingsMode = useMemo(
    () => (
      <div
        className="p-4 space-y-3"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-600 pb-2">
          <h3 className="text-sm font-medium text-white">Voice Settings</h3>
          <button
            onClick={handleToggleMode}
            className="text-gray-400 hover:text-gray-300 transition-colors"
            title="Back to Panel"
          >
            ←
          </button>
        </div>

        {/* Voice settings */}
        <div className="space-y-3">
          <label className="flex items-center justify-between">
            <span className="text-sm text-gray-300">
              Auto-submit on high confidence
            </span>
            <input
              type="checkbox"
              checked={voiceSettings.autoSubmit}
              onChange={(e) =>
                setVoiceSettings((prev) => ({
                  ...prev,
                  autoSubmit: e.target.checked,
                }))
              }
              className="rounded"
            />
          </label>

          <div>
            <label className="block text-sm text-gray-300 mb-1">
              Confidence threshold:{" "}
              {Math.round(voiceSettings.confidenceThreshold * 100)}%
            </label>
            <input
              type="range"
              min="0.5"
              max="1"
              step="0.05"
              value={voiceSettings.confidenceThreshold}
              onChange={(e) =>
                setVoiceSettings((prev) => ({
                  ...prev,
                  confidenceThreshold: parseFloat(e.target.value),
                }))
              }
              className="w-full"
            />
          </div>

          <div>
            <label className="block text-sm text-gray-300 mb-1">
              Voice feedback volume: {Math.round(voiceSettings.volume * 100)}%
            </label>
            <input
              type="range"
              min="0"
              max="1"
              step="0.1"
              value={voiceSettings.volume}
              onChange={(e) =>
                setVoiceSettings((prev) => ({
                  ...prev,
                  volume: parseFloat(e.target.value),
                }))
              }
              className="w-full"
            />
          </div>
        </div>

        {/* Permissions status */}
        <div className="border-t border-gray-600 pt-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-300">
              Accessibility Permissions
            </span>
            <span
              className={`text-xs ${
                panelState.permissionsGranted
                  ? "text-green-400"
                  : "text-red-400"
              }`}
            >
              {panelState.permissionsGranted ? "Granted" : "Required"}
            </span>
          </div>
          {!panelState.permissionsGranted && (
            <p className="text-xs text-gray-400 mt-1">
              Computer use features require accessibility permissions
            </p>
          )}
        </div>
      </div>
    ),
    [voiceSettings, panelState.permissionsGranted, handleToggleMode]
  );

  // Main render logic
  const renderContent = () => {
    switch (panelState.mode) {
      case "compact":
        return renderCompactMode;
      case "expanded":
        return renderExpandedMode;
      case "chat":
        return renderChatMode;
      case "settings":
        return renderSettingsMode;
      case "pill":
        return renderCompactMode;
      default:
        return renderExpandedMode;
    }
  };

  return (
    <div
      className={`
        fixed top-4 right-4 z-50
        bg-black/80 backdrop-blur-sm border border-gray-600/50 rounded-lg
        shadow-2xl shadow-black/50
        transition-all duration-300 ease-in-out
        ${isTransitioning ? "opacity-50" : "opacity-100"}
        ${
          panelState.mode === "compact" || panelState.mode === "pill"
            ? "min-w-[120px]"
            : "min-w-[280px]"
        }
        max-w-sm
        ${
          panelState.mode === "pill"
            ? "pill-container pill-medium pill-medium-blur"
            : ""
        }
      `}
      style={{
        backdropFilter: panelState.mode === "pill" ? "blur(20px)" : "blur(8px)",
        WebkitBackdropFilter:
          panelState.mode === "pill" ? "blur(20px)" : "blur(8px)",
        borderRadius: panelState.mode === "pill" ? "25px" : "8px",
      }}
      onMouseEnter={() => {
        setIsHovered(true);
        if (panelState.mode === "pill") {
          applyVibrancy();
        }
      }}
      onMouseLeave={() => {
        setIsHovered(false);
      }}
      role="dialog"
      aria-label="Voice-controlled agent panel"
      aria-live="polite"
      aria-atomic="true"
    >
      {/* Pill mode controls */}
      {panelState.mode === "pill" && (
        <div className="pill-content">
          <div className="pill-icon">
            {panelState.agentStatus === "listening" && "🎤"}
            {panelState.agentStatus === "thinking" && "🤔"}
            {panelState.agentStatus === "responding" && "💬"}
            {panelState.agentStatus === "idle" && "✨"}
          </div>
          <span className="pill-text">
            {panelState.agentStatus === "idle" && "Ready"}
            {panelState.agentStatus === "listening" && "Listening"}
            {panelState.agentStatus === "thinking" && "Thinking"}
            {panelState.agentStatus === "responding" && "Speaking"}
          </span>

          {/* Quick action buttons */}
          <div className="flex gap-1 ml-2">
            <button
              onClick={() =>
                setPanelState((prev) => ({ ...prev, mode: "expanded" }))
              }
              className="p-1 rounded-full bg-white/10 hover:bg-white/20 transition-colors"
              title="Expand"
            >
              ⤢
            </button>
            <button
              onClick={createPillWindow}
              className="p-1 rounded-full bg-white/10 hover:bg-white/20 transition-colors"
              title="New Pill Window"
            >
              ➕
            </button>
          </div>
        </div>
      )}

      {/* Existing content for other modes */}
      {panelState.mode !== "pill" && renderContent()}
    </div>
  );
}
