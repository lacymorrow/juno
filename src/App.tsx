import { AgentExecutionProgressIndicator } from "@/components/AgentExecutionProgressIndicator"; // Import the AgentExecutionProgressIndicator component
import DevToolsPanel from "@/components/DevToolsPanel";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import { OnboardingFlow } from "@/components/OnboardingFlow";
import { PermissionsFlow } from "@/components/PermissionsFlow";
import { ThinkingMessage } from "@/components/ThinkingMessage";
import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  JsxMessageRenderer,
  isJsxContent,
} from "@/components/ui/jsx-message-renderer";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import { setCurrentAudioElement } from "@/lib/ttsService";
import { cn } from "@/lib/utils";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  Code,
  Copy,
  DogIcon,
  FileText,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Send,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Toaster } from "sonner";
import ClickVisualizer from "./components/ClickVisualizer";
import CommandOverlay from "./components/CommandOverlay";
import KeyPressOverlay from "./components/KeyPressOverlay";
import Settings from "./components/Settings";
import "./styles/globals.css";

// CRITICAL FIX: Add memory management constants
const MAX_CONVERSATION_LENGTH = 100; // Max messages to keep in memory
const MEMORY_CLEANUP_INTERVAL = 30000; // 30 seconds
const MEMORY_PRESSURE_THRESHOLD = 1000; // MB

// Type for conversation messages
type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "thinking"
    | "tool_call_request"
    | "tool_call_result";
  content: string;
  isJsx?: boolean; // Flag to indicate if content should be rendered as JSX
  screenshot_base64?: string; // Optional base64 screenshot data
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean; // For tool call results - indicates if the tool call was successful
  timestamp?: number; // Add timestamp field for message grouping
  isStreaming?: boolean; // Indicates if this message is currently being streamed
  messageId?: string; // Unique identifier for streaming messages
};

// Type for the result from submit_query
type SubmitQueryResult = {
  text: string;
  spoken_text?: string; // Optional separate content for TTS speech
  audio_base64?: string; // Optional base64 audio data
  agent_state: string;
  screenshot_base64?: string; // Optional base64 screenshot data
};

// Type for the backend response event payload
type BackendResponsePayload = {
  query: string;
  response: SubmitQueryResult;
};

// Streaming event types
type StreamingTextEvent = {
  chunk: string;
  message_id?: string;
};

type StreamStartEvent = {
  message_id: string;
};

type StreamEndEvent = {
  message_id: string;
  complete_text: string;
};

// --- Tool Usage Event Type ---
// Note: ToolUsageEntry is defined in DevToolsPanel.tsx where it's actually used

// --- Agent Event Types (mirroring tool_logger.rs) ---
interface ThinkingPayload {
  content: string;
}

interface ToolCallRequestPayload {
  tool_name: string;
  tool_args: any; // Corresponds to serde_json::Value
  content?: string;
}

interface ToolCallResultPayload {
  tool_name: string;
  tool_output: any; // Corresponds to serde_json::Value
  success: boolean;
  content?: string;
  screenshot_base64?: string;
}

interface ScreenshotPayload {
  screenshot_base64: string;
  content?: string;
}

interface GenericContentPayload {
  content: string;
}

// Note: AgentEventPayload union type removed as it's not used - individual payload types are used directly

// This is the structure expected from the `agent-event` emitted by tool_logger.rs
// It matches the Rust `AgentEvent` struct where `event_type` is the `type` field here
// and `payload` is the `payload` field.
// Note: The Rust `AgentEvent` has `event_type` and `payload` as direct fields.
// The `listen` function in Tauri might give us the deserialized payload directly.
// Let's assume the event payload from `listen<AgentEventPayloadTauri>` will be an object
// with `type` and `payload` properties, matching the conceptual structure of Rust's AgentEvent.

interface AgentEventTauri {
  type: string; // "thinking", "tool_call_request", "tool_call_result", "screenshot", "generic_content"
  payload: // This will be one of the specific payload types based on `type`
  | ThinkingPayload
    | ToolCallRequestPayload
    | ToolCallResultPayload
    | ScreenshotPayload
    | GenericContentPayload;
}
// --- End Agent Event Types ---

// Type for view state
type AppView = "chat" | "settings" | "devtools" | "permissions" | "onboarding";

// New modal types for enhanced functionality
type ModalType = "help" | "feedback" | "export" | "import" | "update" | null;

// Enhanced feedback form data
interface FeedbackData {
  type: "issue" | "feature" | "general";
  title: string;
  description: string;
  email?: string;
  priority: "low" | "medium" | "high";
}

// Update check result
interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

// Chat export format
interface ChatExport {
  version: string;
  exported_at: string;
  conversation: ChatMessage[];
  metadata: {
    total_messages: number;
    export_type: "full" | "filtered";
  };
}

// Simple debounce function
function debounce<F extends (...args: any[]) => any>(func: F, waitFor: number) {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  return (...args: Parameters<F>): void => {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => func(...args), waitFor);
  };
}

// Helper function to determine if timestamp should be shown (similar to Slack/Apple Messages)
function shouldShowTimestamp(
  currentMessage: ChatMessage,
  previousMessage: ChatMessage | null,
  timeThresholdMinutes: number = 5
): boolean {
  if (!currentMessage.timestamp) return false;
  if (!previousMessage || !previousMessage.timestamp) return true;

  const timeDiffMinutes =
    (currentMessage.timestamp - previousMessage.timestamp) / (1000 * 60);
  return timeDiffMinutes >= timeThresholdMinutes;
}

// Helper function to format timestamp for display
function formatMessageTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  if (isToday) {
    return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } else {
    return date.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }
}

// Helper function to format full timestamp for title attribute (hover tooltip)
function formatFullTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleString([], {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  });
}

// CRITICAL FIX: Add memory management constants at the top
const MAX_CONVERSATION_LENGTH = 100; // Max messages to keep in memory
const MEMORY_CLEANUP_INTERVAL = 30000; // 30 seconds
const MEMORY_PRESSURE_THRESHOLD = 1000; // MB

// CRITICAL FIX: Add memory monitoring
const useMemoryMonitoring = () => {
  const [memoryPressure, setMemoryPressure] = useState(false);

  useEffect(() => {
    const checkMemory = () => {
      // @ts-ignore - performance.memory is available in Chrome/Edge
      if (performance.memory) {
        const usedJSHeapSize =
          performance.memory.usedJSHeapSize / (1024 * 1024); // MB
        if (usedJSHeapSize > MEMORY_PRESSURE_THRESHOLD) {
          setMemoryPressure(true);
          console.warn(
            `Memory pressure detected: ${usedJSHeapSize.toFixed(2)}MB`
          );
        } else {
          setMemoryPressure(false);
        }
      }
    };

    const interval = setInterval(checkMemory, MEMORY_CLEANUP_INTERVAL);
    return () => clearInterval(interval);
  }, []);

  return memoryPressure;
};

// CRITICAL FIX: Extract audio handling to separate hook
const useAudioManagement = () => {
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(
    null
  );

  const playAudioFromBase64 = useCallback(
    (base64Audio: string) => {
      try {
        // Stop any currently playing audio
        if (currentAudio) {
          currentAudio.pause();
          currentAudio.currentTime = 0;
        }

        const blob = base64ToBlob(base64Audio);
        const audioUrl = URL.createObjectURL(blob);
        const audioElement = new Audio(audioUrl);

        setCurrentAudioElement(audioElement);
        setCurrentAudio(audioElement);

        audioElement.addEventListener("ended", () => {
          URL.revokeObjectURL(audioUrl);
          setCurrentAudio(null);
          setCurrentAudioElement(null);
        });

        audioElement.addEventListener("error", (e) => {
          console.error("Audio playback error:", e);
          URL.revokeObjectURL(audioUrl);
          setCurrentAudio(null);
          setCurrentAudioElement(null);
        });

        audioElement.play().catch((error) => {
          console.error("Failed to play audio:", error);
          URL.revokeObjectURL(audioUrl);
          setCurrentAudio(null);
          setCurrentAudioElement(null);
        });
      } catch (error) {
        console.error("Error processing audio:", error);
      }
    },
    [currentAudio]
  );

  return { currentAudio, playAudioFromBase64 };
};

// CRITICAL FIX: Extract conversation management to separate hook
const useConversationManagement = () => {
  const [conversation, setConversation] = useState<ChatMessage[]>([]);

  // CRITICAL FIX: Add memory-aware conversation management
  const addMessage = useCallback((message: ChatMessage) => {
    setConversation((prev) => {
      const newConversation = [...prev, message];
      // CRITICAL FIX: Trim conversation if too long
      if (newConversation.length > MAX_CONVERSATION_LENGTH) {
        console.warn(
          `Conversation length exceeded ${MAX_CONVERSATION_LENGTH}, trimming older messages`
        );
        return newConversation.slice(-MAX_CONVERSATION_LENGTH);
      }
      return newConversation;
    });
  }, []);

  const updateMessage = useCallback(
    (messageId: string, updates: Partial<ChatMessage>) => {
      setConversation((prev) =>
        prev.map((msg) =>
          msg.messageId === messageId ? { ...msg, ...updates } : msg
        )
      );
    },
    []
  );

  const clearConversation = useCallback(() => {
    setConversation([]);
  }, []);

  return {
    conversation,
    addMessage,
    updateMessage,
    clearConversation,
    setConversation,
  };
};

function App() {
  const [query, setQuery] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  // Helper function to determine notification duration based on level and estimated duration
  const getNotificationDuration = (
    notificationLevel: string,
    estimatedDuration?: string
  ): number => {
    // Base duration by notification level
    const baseDurations = {
      minimal: 1500,
      standard: 3000,
      detailed: 5000,
    };

    const baseDuration =
      baseDurations[notificationLevel as keyof typeof baseDurations] || 3000;

    // Adjust based on estimated duration
    if (estimatedDuration) {
      const durationMultipliers = {
        instant: 0.5,
        short: 0.8,
        medium: 1.0,
        long: 1.5,
      };
      const multiplier =
        durationMultipliers[
          estimatedDuration as keyof typeof durationMultipliers
        ] || 1.0;
      return Math.round(baseDuration * multiplier);
    }

    return baseDuration;
  };

  // Helper function to get notification styling based on tool category
  const getNotificationClassName = (
    toolCategory?: string,
    eventType?: string,
    success?: boolean
  ): string => {
    let className = "tool-notification";

    // Add category-specific styling
    if (toolCategory) {
      className += ` ${toolCategory.toLowerCase()}-category`;
    }

    // Add event type styling
    if (eventType) {
      className += ` ${eventType}-event`;
    }

    // Add success/failure styling for results
    if (eventType === "result" && success !== undefined) {
      className += success ? " success-result" : " failure-result";
    }

    return className;
  };

  // Listen for agent error events to restore input for retry
  useEffect(() => {
    const unlisten = listen<{
      agent_state: string;
      error_message: string;
      original_query: string;
    }>("agent-error", (event) => {
      console.log("Agent error event received:", event.payload);
      const { agent_state, error_message, original_query } = event.payload;

      // Restore the input so user can retry their query
      if (original_query && original_query.trim()) {
        console.log("Restoring input due to agent error:", original_query);
        setQuery(original_query);
      }

      // Also ensure processing state is reset
      setIsProcessing(false);

      // Show error in conversation if not already shown via streaming
      const errorExists = conversation.some(
        (msg) => msg.role === "system" && msg.content.includes(error_message)
      );

      if (!errorExists) {
        const errorMessage: ChatMessage = {
          role: "system",
          content: `Agent ${agent_state.toLowerCase()}: ${error_message}`,
          timestamp: Date.now(),
        };
        setConversation((prev) => [...prev, errorMessage]);
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [conversation]);

  return (
    <main className="h-screen flex flex-col">
      {/* Click Visualizer - overlays the entire app to show click indicators (from tools2) */}
      <ClickVisualizer />
      <KeyPressOverlay />
      <CommandOverlay />

      <div className="w-screen h-screen bg-background text-foreground">
        <div className="container mx-auto p-2 h-full flex flex-col">
          {/* Header */}
          <header className="flex items-center justify-between py-1 px-2 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-1">
                <DogIcon size={16} className="text-blue-500" />
                <span className="text-sm font-semibold">Juno AI</span>
                <div className="flex items-center gap-1">
                  <div
                    className={cn(
                      "w-1.5 h-1.5 rounded-full",
                      serverStatus === "connected"
                        ? "bg-green-500"
                        : serverStatus === "error"
                        ? "bg-red-500"
                        : "bg-yellow-500"
                    )}
                  />
                  {isProcessing && (
                    <div className="text-xs text-muted-foreground">
                      <AgentExecutionProgressIndicator
                        compact
                        className="text-muted-foreground"
                      />
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Voice Status Indicator - only show in chat view */}
            {currentView === "chat" && (
              <div className="flex-1 flex justify-center mx-2">
                <VoiceStatusIndicator
                  variant="compact"
                  className="max-w-xs"
                  showText={false}
                />
              </div>
            )}

            <div className="flex items-center gap-1">
              {/* Back Button - show for settings and devtools views */}
              {(currentView === "settings" ||
                currentView === "devtools" ||
                currentView === "permissions" ||
                currentView === "onboarding") && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setCurrentView("chat")}
                  title="Back to Chat"
                  className="h-7 w-7 p-0"
                >
                  <ArrowLeft size={14} />
                </Button>
              )}
              {/* Toggle Dev Panel Button - only show in chat view */}
              {currentView === "chat" && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setIsDevPanelOpen(!isDevPanelOpen)}
                  title={isDevPanelOpen ? "Hide Dev Panel" : "Show Dev Panel"}
                  className="h-7 w-7 p-0"
                >
                  {isDevPanelOpen ? (
                    <PanelLeftClose size={14} />
                  ) : (
                    <PanelLeftOpen size={14} />
                  )}
                </Button>
              )}
            </div>
          </header>

          {/* Main Content Area - Conditional based on current view */}
          {currentView === "settings" ? (
            <div className="flex-grow rounded-lg border overflow-hidden">
              <ScrollArea className="h-full w-full">
                <Settings
                  onNavigateToDevTools={() => setCurrentView("devtools")}
                  onNavigateToChat={() => setCurrentView("chat")}
                  onNavigateToPermissions={() => setCurrentView("permissions")}
                />
              </ScrollArea>
            </div>
          ) : currentView === "devtools" ? (
            <div className="flex-grow rounded-lg border overflow-hidden">
              <ScrollArea className="h-full w-full p-2">
                <h2 className="text-sm font-semibold mb-2 border-b pb-1">
                  Developer Tools & Logs
                </h2>
                <DevToolsPanel />
              </ScrollArea>
            </div>
          ) : currentView === "permissions" ? (
            <div className="flex-grow rounded-lg border overflow-hidden">
              <ScrollArea className="h-full w-full p-2">
                <PermissionsFlow
                  onComplete={() => {
                    setShowPermissionsFlow(false);
                    setCurrentView("chat");
                  }}
                  onSkip={() => {
                    setShowPermissionsFlow(false);
                    setCurrentView("chat");
                  }}
                  showSkipOption={true}
                  className="max-w-4xl mx-auto"
                />
              </ScrollArea>
            </div>
          ) : currentView === "onboarding" ? (
            <div className="flex-grow rounded-lg border overflow-hidden">
              <ScrollArea className="h-full w-full p-2">
                <OnboardingFlow
                  onComplete={handleOnboardingComplete}
                  onSkip={handleOnboardingSkip}
                  permissionsAlreadyGranted={permissionsGranted}
                />
              </ScrollArea>
            </div>
          ) : (
            <ResizablePanelGroup
              direction="horizontal"
              className="flex-grow rounded-lg border overflow-hidden"
            >
              {/* Chat Panel */}
              <ResizablePanel
                defaultSize={isDevPanelOpen ? 50 : 100}
                minSize={30}
              >
                <div className="flex flex-col h-full p-2">
                  {/* Conversation Area */}
                  <ScrollArea className="flex-1 min-h-0 mb-2 -mr-4 pr-4">
                    {conversation.length === 0 ? (
                      /* Compact welcome message when conversation is empty */
                      <div className="flex flex-col items-center justify-center h-full text-center space-y-2 p-2">
                        <div className="space-y-1">
                          <DogIcon
                            size={16}
                            className="text-blue-500 mx-auto"
                          />
                          <div>
                            <h2 className="text-sm font-semibold">Juno AI</h2>
                            <p className="text-xs text-muted-foreground">
                              AI desktop assistant
                            </p>
                          </div>
                        </div>

                        {/* Compact Example Prompts */}
                        <ExamplePrompts
                          onPromptSelect={handleExamplePromptSelect}
                        />
                      </div>
                    ) : (
                      conversation.map((msg, index) => {
                        const previousMsg =
                          index > 0 ? conversation[index - 1] : null;
                        const showTimestamp = shouldShowTimestamp(
                          msg,
                          previousMsg
                        );

                        return (
                          <div
                            key={`msg-${index}-${msg.timestamp || Date.now()}`}
                          >
                            {/* Timestamp header - show when needed, similar to Slack/Apple Messages */}
                            {showTimestamp && msg.timestamp && (
                              <div className="flex justify-center my-4">
                                <span
                                  className="text-xs text-muted-foreground bg-background px-3 py-1 border rounded-full shadow-sm cursor-default"
                                  title={formatFullTimestamp(msg.timestamp)}
                                >
                                  {formatMessageTimestamp(msg.timestamp)}
                                </span>
                              </div>
                            )}

                            {/* Handle thinking messages with special component */}
                            {msg.role === "thinking" ? (
                              <div className="flex justify-start">
                                <ThinkingMessage
                                  content={msg.content}
                                  timestamp={msg.timestamp}
                                />
                              </div>
                            ) : msg.role === "tool_call_request" ? (
                              <div className="flex justify-start">
                                <ToolCallRequest
                                  toolName={msg.tool_name || "unknown"}
                                  toolArgs={msg.tool_args}
                                  content={msg.content}
                                  timestamp={msg.timestamp}
                                />
                              </div>
                            ) : msg.role === "tool_call_result" ? (
                              <div className="flex justify-start">
                                <ToolCallResult
                                  toolName={msg.tool_name || "unknown"}
                                  toolOutput={msg.tool_output}
                                  success={msg.success ?? true} // Default to true if not specified
                                  content={msg.content}
                                  screenshot_base64={msg.screenshot_base64}
                                  timestamp={msg.timestamp}
                                />
                              </div>
                            ) : (
                              <div
                                className={`mb-3 flex ${
                                  msg.role === "user"
                                    ? "justify-end"
                                    : "justify-start"
                                }`}
                              >
                                <div className="relative group max-w-[85%]">
                                  <span
                                    className={cn(
                                      "inline-block w-full px-3 py-1.5 rounded-lg shadow-sm",
                                      msg.role === "user"
                                        ? "bg-primary text-primary-foreground"
                                        : msg.role === "assistant"
                                        ? "bg-muted"
                                        : msg.role === "system" &&
                                          msg.screenshot_base64
                                        ? "bg-muted/80 border border-primary/20 p-2"
                                        : "bg-secondary text-secondary-foreground text-xs italic opacity-80" // Default system
                                    )}
                                  >
                                    {msg.role === "assistant" &&
                                    (!msg.content ||
                                      msg.content.trim() === "") ? (
                                      <span className="text-muted-foreground italic flex items-center gap-2">
                                        <span>✓</span>
                                        <span>Task completed successfully</span>
                                      </span>
                                    ) : msg.isJsx ||
                                      (msg.role === "assistant" &&
                                        !msg.isStreaming &&
                                        isJsxContent(msg.content)) ? (
                                      <JsxMessageRenderer jsx={msg.content} />
                                    ) : (
                                      msg.content
                                    )}
                                    {msg.screenshot_base64 && (
                                      <div
                                        className={cn(
                                          "mt-2",
                                          msg.role !== "system" &&
                                            "border-t pt-2"
                                        )}
                                      >
                                        <div className="text-xs text-muted-foreground mb-1">
                                          {msg.role === "system"
                                            ? "Screenshot captured by AI:"
                                            : "Screenshot:"}
                                        </div>
                                        <div className="relative">
                                          <img
                                            src={`data:image/png;base64,${msg.screenshot_base64}`}
                                            alt="Screenshot"
                                            className="rounded w-full object-contain max-h-[300px] border border-border shadow-sm"
                                          />
                                          <div className="absolute inset-0 bg-gradient-to-t from-background/20 to-transparent pointer-events-none"></div>
                                        </div>
                                      </div>
                                    )}
                                    {/* Show typing indicator for streaming messages */}
                                    {msg.isStreaming && (
                                      <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse">
                                        |
                                      </span>
                                    )}
                                  </span>

                                  {/* Action buttons for assistant messages with content - positioned at bottom */}
                                  {msg.role === "assistant" &&
                                    msg.content &&
                                    msg.content.trim() !== "" &&
                                    !msg.isStreaming && (
                                      <div className="mt-2 pt-2 border-t border-border/50 opacity-0 group-hover:opacity-100 transition-all duration-200 flex justify-end gap-2">
                                        <div className="flex gap-1 bg-background/90 backdrop-blur-sm rounded-md p-1 shadow-sm border">
                                          <Button
                                            variant="ghost"
                                            size="sm"
                                            className={cn(
                                              "h-7 w-7 p-0 transition-all duration-150 relative",
                                              copyingMessageId ===
                                                `copy-${index}`
                                                ? "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300 scale-95"
                                                : "hover:bg-blue-50 hover:text-blue-600 dark:hover:bg-blue-950 dark:hover:text-blue-400 hover:scale-105"
                                            )}
                                            onClick={() =>
                                              handleCopyResponse(
                                                msg.content,
                                                index
                                              )
                                            }
                                            disabled={
                                              copyingMessageId ===
                                              `copy-${index}`
                                            }
                                            title={
                                              copyingMessageId ===
                                              `copy-${index}`
                                                ? "Copying..."
                                                : "Copy response to clipboard"
                                            }
                                          >
                                            {copyingMessageId ===
                                            `copy-${index}` ? (
                                              <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                                            ) : (
                                              <Copy size={14} />
                                            )}
                                          </Button>
                                          <Button
                                            variant="ghost"
                                            size="sm"
                                            className={cn(
                                              "h-7 w-7 p-0 transition-all duration-150 relative",
                                              savingMessageId ===
                                                `save-html-${index}`
                                                ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 scale-95"
                                                : "hover:bg-green-50 hover:text-green-600 dark:hover:bg-green-950 dark:hover:text-green-400 hover:scale-105"
                                            )}
                                            onClick={() =>
                                              handleSaveResponse(
                                                msg.content,
                                                "html",
                                                index
                                              )
                                            }
                                            disabled={
                                              savingMessageId ===
                                              `save-html-${index}`
                                            }
                                            title={
                                              savingMessageId ===
                                              `save-html-${index}`
                                                ? "Saving HTML..."
                                                : "Save as HTML file with professional styling"
                                            }
                                          >
                                            {savingMessageId ===
                                            `save-html-${index}` ? (
                                              <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                                            ) : (
                                              <Code size={14} />
                                            )}
                                          </Button>
                                          <Button
                                            variant="ghost"
                                            size="sm"
                                            className={cn(
                                              "h-7 w-7 p-0 transition-all duration-150 relative",
                                              savingMessageId ===
                                                `save-markdown-${index}`
                                                ? "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300 scale-95"
                                                : "hover:bg-purple-50 hover:text-purple-600 dark:hover:bg-purple-950 dark:hover:text-purple-400 hover:scale-105"
                                            )}
                                            onClick={() =>
                                              handleSaveResponse(
                                                msg.content,
                                                "markdown",
                                                index
                                              )
                                            }
                                            disabled={
                                              savingMessageId ===
                                              `save-markdown-${index}`
                                            }
                                            title={
                                              savingMessageId ===
                                              `save-markdown-${index}`
                                                ? "Saving Markdown..."
                                                : "Save as Markdown file for documentation"
                                            }
                                          >
                                            {savingMessageId ===
                                            `save-markdown-${index}` ? (
                                              <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                                            ) : (
                                              <FileText size={14} />
                                            )}
                                          </Button>
                                        </div>
                                      </div>
                                    )}
                                </div>
                              </div>
                            )}
                          </div>
                        );
                      })
                    )}
                    <div ref={conversationEndRef} />
                  </ScrollArea>

                  {/* Input Form */}
                  <form
                    onSubmit={handleSubmit}
                    className="flex gap-2 flex-shrink-0 mt-auto"
                  >
                    <Input
                      type="text"
                      placeholder={
                        isProcessing ? "Processing..." : "Enter your query..."
                      }
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      disabled={isProcessing || serverStatus !== "connected"}
                      className="flex-grow"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={startNewChat}
                      disabled={isProcessing}
                      title="Start new agent chat"
                    >
                      <Plus size={18} />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      onClick={clearConversation}
                      disabled={isProcessing}
                      title="Clear conversation history"
                    >
                      <Trash2 size={18} />
                    </Button>
                    <Button
                      type="submit"
                      disabled={
                        isProcessing ||
                        serverStatus !== "connected" ||
                        !query.trim()
                      }
                    >
                      <Send size={18} />
                    </Button>
                  </form>
                </div>
              </ResizablePanel>

              {/* Conditionally render the resizable handle and dev panel based on isDevPanelOpen */}
              {isDevPanelOpen && (
                <>
                  {/* Resizable Handle */}
                  <ResizableHandle withHandle />

                  {/* Dev Tools & Logs Panel */}
                  <ResizablePanel defaultSize={50} minSize={25}>
                    <ScrollArea className="h-full w-full p-2">
                      {/* Title (replaces CardHeader) */}
                      <h2 className="text-sm font-semibold mb-2 border-b pb-1">
                        Developer Tools & Logs
                      </h2>
                      {/* DevToolsPanel Component */}
                      <div className="border-b pb-2 mb-2">
                        <DevToolsPanel />
                      </div>
                      {/* Logs Area */}
                      <div className="flex-grow">{/* Logs Area */}</div>
                    </ScrollArea>
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
          )}
        </div>
      </div>

      {/* Enhanced modal system */}
      {renderModal()}

      {/* Update check loading indicator */}
      {isCheckingUpdate && (
        <div className="fixed bottom-4 right-4 bg-blue-500 text-white px-4 py-2 rounded-lg shadow-lg">
          Checking for updates...
        </div>
      )}

      {/* Version display in bottom left corner */}
      {appVersion && (
        <div className="fixed bottom-2 left-2 text-xs text-muted-foreground/50 pointer-events-none select-none">
          {appVersion}
        </div>
      )}

      {/* Toast notifications */}
      <Toaster
        position="bottom-right"
        expand={true}
        richColors={true}
        closeButton={true}
        duration={5000}
      />
    </main>
  );
}

export default App;
