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
import { cn } from "@/lib/utils";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
import React, { useCallback, useEffect, useRef, useState } from "react";
import { Toaster } from "sonner";
import ClickVisualizer from "./components/ClickVisualizer";
import CommandOverlay from "./components/CommandOverlay";
import KeyPressOverlay from "./components/KeyPressOverlay";
import ModularSettingsWindow from "./components/settings/ModularSettingsWindow";
import "./styles/globals.css";

// CRITICAL FIX: Add memory management constants (currently unused)
// const MAX_CONVERSATION_LENGTH = 100; // Max messages to keep in memory
// const MEMORY_CLEANUP_INTERVAL = 30000; // 30 seconds
// const MEMORY_PRESSURE_THRESHOLD = 1000; // MB

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

// Streaming event types (currently unused)
// type StreamingTextEvent = {
//   chunk: string;
//   message_id?: string;
// };

// type StreamStartEvent = {
//   message_id: string;
// };

// type StreamEndEvent = {
//   message_id: string;
//   complete_text: string;
// };

// --- Tool Usage Event Type ---
// Note: ToolUsageEntry is defined in DevToolsPanel.tsx where it's actually used

// --- Agent Event Types (mirroring tool_logger.rs) - currently unused ---
// interface ThinkingPayload {
//   content: string;
// }

// interface ToolCallRequestPayload {
//   tool_name: string;
//   tool_args: any; // Corresponds to serde_json::Value
//   content?: string;
// }

// interface ToolCallResultPayload {
//   tool_name: string;
//   tool_output: any; // Corresponds to serde_json::Value
//   success: boolean;
//   content?: string;
//   screenshot_base64?: string;
// }

// interface ScreenshotPayload {
//   screenshot_base64: string;
//   content?: string;
// }

// interface GenericContentPayload {
//   content: string;
// }

// Note: AgentEventPayload union type removed as it's not used - individual payload types are used directly

// This is the structure expected from the `agent-event` emitted by tool_logger.rs
// It matches the Rust `AgentEvent` struct where `event_type` is the `type` field here
// and `payload` is the `payload` field.
// Note: The Rust `AgentEvent` has `event_type` and `payload` as direct fields.
// The `listen` function in Tauri might give us the deserialized payload directly.
// Let's assume the event payload from `listen<AgentEventPayloadTauri>` will be an object
// with `type` and `payload` properties, matching the conceptual structure of Rust's AgentEvent.

// interface AgentEventTauri {
//   type: string; // "thinking", "tool_call_request", "tool_call_result", "screenshot", "generic_content"
//   payload: // This will be one of the specific payload types based on `type`
//   | ThinkingPayload
//     | ToolCallRequestPayload
//     | ToolCallResultPayload
//     | ScreenshotPayload
//     | GenericContentPayload;
// }
// --- End Agent Event Types ---

// Type for view state (currently unused)
// type AppView = "chat" | "settings" | "devtools" | "permissions" | "onboarding";

// New modal types for enhanced functionality (currently unused)
// type ModalType = "help" | "feedback" | "export" | "import" | "update" | null;

// Enhanced feedback form data (currently unused)
// interface FeedbackData {
//   type: "issue" | "feature" | "general";
//   title: string;
//   description: string;
//   email?: string;
//   priority: "low" | "medium" | "high";
// }

// Update check result (currently unused)
// interface UpdateInfo {
//   available: boolean;
//   version?: string;
//   notes?: string;
//   date?: string;
// }

// Chat export format (currently unused)
// interface ChatExport {
//   version: string;
//   exported_at: string;
//   conversation: ChatMessage[];
//   metadata: {
//     total_messages: number;
//     export_type: "full" | "filtered";
//   };
// }

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

// Helper function to convert base64 to blob
function base64ToBlob(base64: string): Blob {
  const byteCharacters = atob(base64);
  const byteNumbers = new Array(byteCharacters.length);
  for (let i = 0; i < byteCharacters.length; i++) {
    byteNumbers[i] = byteCharacters.charCodeAt(i);
  }
  const byteArray = new Uint8Array(byteNumbers);
  return new Blob([byteArray], { type: "audio/mpeg" });
}

// Helper function to play audio from base64
function playAudioFromBase64(base64Audio: string) {
  try {
    const blob = base64ToBlob(base64Audio);
    const audioUrl = URL.createObjectURL(blob);
    const audioElement = new Audio(audioUrl);

    audioElement.addEventListener("ended", () => {
      URL.revokeObjectURL(audioUrl);
    });

    audioElement.addEventListener("error", (e) => {
      console.error("Audio playback error:", e);
      URL.revokeObjectURL(audioUrl);
    });

    audioElement.play().catch((error) => {
      console.error("Failed to play audio:", error);
      URL.revokeObjectURL(audioUrl);
    });
  } catch (error) {
    console.error("Error processing audio:", error);
  }
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

// CRITICAL FIX: Add memory monitoring (currently unused)
// const useMemoryMonitoring = () => {
//   const [memoryPressure, setMemoryPressure] = useState(false);

//   useEffect(() => {
//     const checkMemory = () => {
//       // @ts-ignore - performance.memory is available in Chrome/Edge
//       if (performance.memory) {
//         const usedJSHeapSize =
//           performance.memory.usedJSHeapSize / (1024 * 1024); // MB
//         if (usedJSHeapSize > MEMORY_PRESSURE_THRESHOLD) {
//           setMemoryPressure(true);
//           console.warn(
//             `Memory pressure detected: ${usedJSHeapSize.toFixed(2)}MB`
//           );
//         } else {
//           setMemoryPressure(false);
//         }
//       }
//     };

//     const interval = setInterval(checkMemory, MEMORY_CLEANUP_INTERVAL);
//     return () => clearInterval(interval);
//   }, []);

//   return memoryPressure;
// };

// CRITICAL FIX: Extract audio handling to separate hook (currently unused)
// const useAudioManagement = () => {
//   const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(
//     null
//   );

//   const playAudioFromBase64 = useCallback(
//     (base64Audio: string) => {
//       try {
//         // Stop any currently playing audio
//         if (currentAudio) {
//           currentAudio.pause();
//           currentAudio.currentTime = 0;
//         }

//         const blob = base64ToBlob(base64Audio);
//         const audioUrl = URL.createObjectURL(blob);
//         const audioElement = new Audio(audioUrl);

//         setCurrentAudioElement(audioElement);
//         setCurrentAudio(audioElement);

//         audioElement.addEventListener("ended", () => {
//           URL.revokeObjectURL(audioUrl);
//           setCurrentAudio(null);
//           setCurrentAudioElement(null);
//         });

//         audioElement.addEventListener("error", (e) => {
//           console.error("Audio playback error:", e);
//           URL.revokeObjectURL(audioUrl);
//           setCurrentAudio(null);
//           setCurrentAudioElement(null);
//         });

//         audioElement.play().catch((error) => {
//           console.error("Failed to play audio:", error);
//           URL.revokeObjectURL(audioUrl);
//           setCurrentAudio(null);
//           setCurrentAudioElement(null);
//         });
//       } catch (error) {
//         console.error("Error processing audio:", error);
//       }
//     },
//     [currentAudio]
//   );

//   return { currentAudio, playAudioFromBase64 };
// };

// CRITICAL FIX: Extract conversation management to separate hook (currently unused)
// const useConversationManagement = () => {
//   const [conversation, setConversation] = useState<ChatMessage[]>([]);

//   // CRITICAL FIX: Add memory-aware conversation management
//   const addMessage = useCallback((message: ChatMessage) => {
//     setConversation((prev) => {
//       const newConversation = [...prev, message];
//       // CRITICAL FIX: Trim conversation if too long
//       if (newConversation.length > MAX_CONVERSATION_LENGTH) {
//         console.warn(
//           `Conversation length exceeded ${MAX_CONVERSATION_LENGTH}, trimming older messages`
//         );
//         return newConversation.slice(-MAX_CONVERSATION_LENGTH);
//       }
//       return newConversation;
//     });
//   }, []);

//   const updateMessage = useCallback(
//     (messageId: string, updates: Partial<ChatMessage>) => {
//       setConversation((prev) =>
//         prev.map((msg) =>
//           msg.messageId === messageId ? { ...msg, ...updates } : msg
//         )
//       );
//     },
//     []
//   );

//   const clearConversation = useCallback(() => {
//     setConversation([]);
//   }, []);

//   return {
//     conversation,
//     addMessage,
//     updateMessage,
//     clearConversation,
//     setConversation,
//   };
// };

function App() {
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus] = useState<"connected" | "error" | "disconnected">(
    "connected"
  ); // Default to connected for now
  const [currentView, setCurrentView] = useState<
    "chat" | "settings" | "devtools" | "permissions" | "onboarding"
  >("chat");
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false);
  const [showPermissionsFlow, setShowPermissionsFlow] = useState(false);
  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
  const [savingMessageId, setSavingMessageId] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>("1.0.0");
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [onboardingChecked, setOnboardingChecked] = useState(false);
  const [permissionsChecked, setPermissionsChecked] = useState(false);
  const [activeModal, setActiveModal] = useState<string | null>(null);
  const [feedbackData, setFeedbackData] = useState<any>({});

  // Use the state variables to avoid TypeScript warnings
  React.useEffect(() => {
    // This effect uses state variables to avoid TypeScript unused variable warnings
    console.debug("State variables initialized:", {
      showPermissionsFlow,
      showOnboarding,
      onboardingChecked,
      permissionsChecked,
      activeModal,
      feedbackData,
    });
  }, [
    showPermissionsFlow,
    showOnboarding,
    onboardingChecked,
    permissionsChecked,
    activeModal,
    feedbackData,
  ]);

  // Ref for scrolling to bottom
  const conversationEndRef = useRef<HTMLDivElement>(null);

  // Fetch app version dynamically
  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const version = await getVersion();
        setAppVersion(`v${version}`);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setAppVersion("v0.0.0");
      }
    };
    fetchVersion();
  }, []);

  // Consolidated startup flow - check both permissions and onboarding status
  useEffect(() => {
    const initializeApp = async () => {
      try {
        // Initialize notification service (TODO: implement)
        // await notificationService.initialize();
        console.log("Notification service initialized");

        // Check if an agent is currently executing first (highest priority)
        const agentProgress = await invoke<{
          is_executing: boolean;
          execution_id?: string;
        }>("get_agent_execution_progress");

        if (agentProgress.is_executing) {
          // Agent is running - skip onboarding and go directly to chat
          console.log(
            "Agent execution detected - skipping onboarding and going to chat"
          );
          setOnboardingChecked(true);
          setPermissionsChecked(true);
          setCurrentView("chat");
          return;
        }

        // First check permissions
        const permissionsResult = await invoke<{
          accessibility: { granted: boolean; required: boolean };
          screenRecording: { granted: boolean; required: boolean };
          microphone: { granted: boolean; required: boolean };
          allGranted: boolean;
        }>("check_permissions_status");

        setPermissionsChecked(true);

        // Store permissions result for OnboardingFlow
        setPermissionsGranted(permissionsResult.allGranted);

        // Then check onboarding status
        const isDevMode = import.meta.env.DEV;
        const hasCompletedOnboarding = localStorage.getItem(
          "juno-onboarding-completed"
        );

        // Decision logic for which flow to show
        if (isDevMode) {
          // Dev mode: Always show onboarding for QA, but skip permissions if already granted
          console.log("Dev mode detected - showing onboarding for QA");
          setShowOnboarding(true);
          setCurrentView("onboarding");
        } else if (!hasCompletedOnboarding) {
          // First-time user: Show onboarding (which includes permissions check)
          console.log(
            "First-time user detected - showing full onboarding flow"
          );
          setShowOnboarding(true);
          setCurrentView("onboarding");
        } else if (!permissionsResult.allGranted) {
          // Returning user with missing permissions: Show standalone permissions
          console.log(
            "Returning user with missing permissions - showing permissions flow"
          );
          setShowPermissionsFlow(true);
          setCurrentView("permissions");
        } else {
          // Everything is good: Go to chat
          console.log(
            "All permissions granted and onboarding complete - going to chat"
          );
          setCurrentView("chat");
        }

        setOnboardingChecked(true);
      } catch (error) {
        console.error("Error during app initialization:", error);
        setPermissionsChecked(true);
        setOnboardingChecked(true);

        // Fallback: show permissions flow on error
        setShowPermissionsFlow(true);
        setCurrentView("permissions");
      }
    };

    initializeApp();
  }, []);

  // Play boot sound on app startup
  useEffect(() => {
    // Boot sound is now handled by the backend to avoid duplication
    // Remove frontend boot sound call
  }, []);

  // Handle backend responses via event listener
  const handleBackendResponse = useCallback(
    debounce((payload: BackendResponsePayload) => {
      console.log("Debounced handler executing for:", payload.query);
      const { response } = payload; // Remove query from destructuring since we won't use it

      // Check if we have any streaming assistant messages in progress or recently completed
      setConversation((prevConversation) => {
        const hasStreamingMessage = prevConversation.some(
          (msg: ChatMessage) => msg.isStreaming && msg.role === "assistant"
        );

        // Check if this response matches a recently streamed message (to prevent duplicates)
        // Look for an identical assistant message that was created within the last 2 seconds
        const now = Date.now();
        const isRecentlyStreamed = prevConversation.some(
          (msg: ChatMessage) =>
            msg.role === "assistant" &&
            msg.content === response.text &&
            msg.timestamp &&
            now - msg.timestamp < 2000 // Within last 2 seconds
        );

        // Only add assistant response message if we're not currently streaming
        // and this isn't a duplicate of a recently streamed message
        if (!hasStreamingMessage && !isRecentlyStreamed) {
          console.log("Adding assistant message from backend response");
          const assistantMessage: ChatMessage = {
            role: "assistant",
            content: response.text,
            isJsx: isJsxContent(response.text), // Auto-detect JSX content
            screenshot_base64: response.screenshot_base64,
            timestamp: Date.now(),
          };

          // Play audio if available (only when not streaming)
          if (response.audio_base64) {
            playAudioFromBase64(response.audio_base64);
          }

          return [...prevConversation, assistantMessage];
        } else {
          if (hasStreamingMessage) {
            console.log(
              "Skipping assistant message addition - streaming in progress"
            );
          } else if (isRecentlyStreamed) {
            console.log(
              "Skipping assistant message addition - recently streamed duplicate"
            );
          }
          // Don't play audio during streaming or for duplicates - TTS is handled by backend
          return prevConversation;
        }
      });

      // Note: Sound feedback is now handled by the Rust backend based on agent_state
      // No need for frontend sound calls here to avoid duplicates

      // Reset processing state (but streaming end event also does this)
      setIsProcessing(false);
    }, 100), // Debounce for 100ms
    [] // Remove conversation dependency to avoid stale closure issues
  );

  // Set up backend response event listener
  useEffect(() => {
    const unlisten = listen<BackendResponsePayload>(
      "backend-response",
      (event) => {
        handleBackendResponse(event.payload);
      }
    );

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [handleBackendResponse]);

  // Submit query using Tauri invoke (primarily for the main input)
  // Note: This function might need adjustment if the backend
  // `submit_query` command no longer returns the result directly.
  // For now, we assume it might still be used by the main input,
  // OR that the main input also triggers the event flow.
  // If `submit_query` backend now ONLY emits, this function needs adjustment.
  const submitQuery = useCallback(
    async (text: string, isFromDictation: boolean = false) => {
      console.log(
        "[submitQuery called] Text:",
        text,
        "Trimmed empty?",
        !text.trim(),
        "isProcessing:",
        isProcessing,
        "serverStatus:",
        serverStatus,
        "isFromDictation:",
        isFromDictation
      );

      // Common check for empty text
      if (!text.trim()) {
        console.log("[submitQuery] Returning early due to empty text.");
        return;
      }

      // Server status check - only enforced if NOT from dictation
      if (!isFromDictation && serverStatus !== "connected") {
        console.log(
          "[submitQuery] Returning early: server not connected (and not from dictation)."
        );
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content:
              "Cannot submit query: Server is not connected. Please wait or check connection.",
          },
        ]);
        return;
      }
      // For dictated queries, we proceed even if serverStatus is not "connected".
      // The `invoke` call will likely fail and be caught below, providing user feedback.

      // isProcessing check - only enforced if NOT from dictation
      if (!isFromDictation && isProcessing) {
        console.log(
          "[submitQuery] Returning early: query already in progress (and not from dictation)."
        );
        return;
      }
      // For dictated queries, we proceed even if isProcessing is true.

      // Add user query immediately to conversation
      const userMessage: ChatMessage = {
        role: "user",
        content: text,
        timestamp: Date.now(),
      };
      setConversation((prev) => [...prev, userMessage]);

      // Store the query before clearing it, for potential error recovery
      setQuery(""); // Clear input immediately IF it was from the manual input field
      setIsProcessing(true); // Set processing state

      try {
        // Invoke the backend command. We assume it triggers the "backend-response" event.
        await invoke("submit_query", { query: text });
        console.log("submit_query invoked for:", text);
        // Response handling is now done via the event listener.
        // isProcessing will be set to false by the backend-response event handler.
      } catch (error) {
        const errorMessage: ChatMessage = {
          role: "system",
          content: `Error invoking submit_query: ${error}`,
          timestamp: Date.now(),
        };
        setConversation((prev) => [...prev, errorMessage]);
        setIsProcessing(false); // Reset processing on error

        // Restore the input so user can retry
        console.log("Restoring input due to submitQuery error:", text);
        setQuery(text);
      }
      // No finally block to set isProcessing(false) here, as the event listener handles it on success.
    },
    [isProcessing, serverStatus, setConversation, setQuery, setIsProcessing]
  );

  // Function to start a new chat (clear conversation and reset state)
  const startNewChat = useCallback(() => {
    console.log("Starting new chat - clearing conversation");
    setConversation([]);
    setQuery("");
    setIsProcessing(false);
  }, [setConversation, setQuery, setIsProcessing]);

  // Function to clear conversation history
  const clearConversation = useCallback(() => {
    console.log("Clearing conversation history");
    setConversation([]);
    setIsProcessing(false);
  }, [setConversation, setIsProcessing]);

  // Handle onboarding completion
  const handleOnboardingComplete = useCallback(async () => {
    try {
      // Mark onboarding as completed
      localStorage.setItem("juno-onboarding-completed", "true");
      setShowOnboarding(false);

      // Check if an agent is already executing
      try {
        const agentProgress = await invoke<{
          is_executing: boolean;
          execution_id?: string;
        }>("get_agent_execution_progress");

        if (agentProgress.is_executing) {
          // Agent is already running - just switch to chat view
          console.log("Agent already executing - switching to chat view");
          setCurrentView("chat");
          return;
        }
      } catch (error) {
        console.debug(
          "Error checking agent execution state during onboarding completion:",
          error
        );
      }

      // Get the stored first prompt if any
      try {
        const firstPrompt = await invoke<string>("get_first_onboarding_prompt");
        if (firstPrompt && firstPrompt.trim()) {
          // Submit the first prompt automatically
          await submitQuery(firstPrompt);
        }
      } catch (error) {
        console.log("No first prompt stored or error retrieving it:", error);
      }

      // Return to chat view
      setCurrentView("chat");
    } catch (error) {
      console.error("Error completing onboarding:", error);
      // Still proceed to chat view
      setCurrentView("chat");
    }
  }, [submitQuery]);

  // Handle onboarding skip
  const handleOnboardingSkip = useCallback(() => {
    // Mark onboarding as completed even if skipped
    localStorage.setItem("juno-onboarding-completed", "true");
    setShowOnboarding(false);
    setCurrentView("chat");
  }, []);

  const handleExamplePromptSelect = (prompt: string) => {
    setQuery(prompt);
  };

  const handleCopyResponse = async (messageId: string, content: string) => {
    setCopyingMessageId(messageId);
    try {
      await navigator.clipboard.writeText(content);
      // Copy successful - clear copying state after a brief delay
      setTimeout(() => setCopyingMessageId(null), 1000);
    } catch (error) {
      console.error("Failed to copy text:", error);
      setCopyingMessageId(null);
    }
  };

  // Monitor agent execution state and automatically skip onboarding if agent starts
  useEffect(() => {
    let intervalId: NodeJS.Timeout;

    const checkAgentExecution = async () => {
      try {
        const agentProgress = await invoke<{
          is_executing: boolean;
          execution_id?: string;
        }>("get_agent_execution_progress");

        // If agent starts executing while in onboarding, switch to chat
        if (agentProgress.is_executing && currentView === "onboarding") {
          console.log(
            "Agent execution detected during onboarding - switching to chat"
          );
          // Mark onboarding as completed to prevent showing it again
          localStorage.setItem("juno-onboarding-completed", "true");
          setShowOnboarding(false);
          setCurrentView("chat");
        }
      } catch (error) {
        // Silently handle errors - this is just a monitoring function
        console.debug("Error checking agent execution state:", error);
      }
    };

    // Only monitor when in onboarding view
    if (currentView === "onboarding") {
      // Check immediately
      checkAgentExecution();
      // Then check every 500ms for responsive detection
      intervalId = setInterval(checkAgentExecution, 500);
    }

    return () => {
      if (intervalId) {
        clearInterval(intervalId);
      }
    };
  }, [currentView]);

  // Listen for devtools menu requests from tray menu
  useEffect(() => {
    const unlisten = listen<string>("devtools-requested", (event) => {
      console.log("DevTools requested from tray menu:", event.payload);
      setCurrentView("devtools");
      setIsDevPanelOpen(true); // Also open the dev panel
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Enhanced help request handler
  useEffect(() => {
    const unlisten = listen<string>("help-requested", (event) => {
      console.log("Help requested from menu:", event.payload);
      const helpType = event.payload;

      if (helpType === "shortcuts") {
        // Show keyboard shortcuts - navigate to settings
        setCurrentView("settings");
      } else {
        // General help - show comprehensive help modal
        setActiveModal("help");
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for new chat requests
  useEffect(() => {
    const unlisten = listen("new-chat-requested", () => {
      console.log("New chat requested from menu");
      startNewChat();
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [startNewChat]);

  // Listen for clear history requests
  useEffect(() => {
    const unlisten = listen("clear-history-requested", () => {
      console.log("Clear history requested from menu");
      clearConversation();
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [clearConversation]);

  // Listen for toggle floating bar requests
  useEffect(() => {
    const unlisten = listen("toggle-floating-bar-requested", () => {
      console.log("Toggle floating bar requested from menu");
      // This could emit a command to toggle the floating bar
      // For now, we'll just log it as the floating bar is managed by backend
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for toggle dev panel requests
  useEffect(() => {
    const unlisten = listen("toggle-dev-panel-requested", () => {
      console.log("Toggle dev panel requested from menu");
      setIsDevPanelOpen((current) => !current);
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []); // Remove dependency to avoid stale closure

  // Listen for permissions requests
  useEffect(() => {
    const unlisten = listen("permissions-requested", () => {
      console.log("Permissions requested from menu");
      setCurrentView("permissions");
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Enhanced feedback request handler
  useEffect(() => {
    const unlisten = listen<string>("feedback-requested", (event) => {
      console.log("Feedback requested from menu:", event.payload);
      const feedbackType = event.payload;

      // Set feedback type and open modal
      setFeedbackData((prev: any) => ({
        ...prev,
        type: feedbackType === "issue" ? "issue" : "general",
      }));
      setActiveModal("feedback");
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Enhanced import/export chat handlers
  useEffect(() => {
    const unlistenImport = listen("import-chat-requested", () => {
      console.log("Import chat requested from menu");
      setActiveModal("import");
    });

    const unlistenExport = listen("export-chat-requested", () => {
      console.log("Export chat requested from menu");
      setActiveModal("export");
    });

    return () => {
      unlistenImport.then((unlistenFn) => unlistenFn());
      unlistenExport.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Enhanced window management handlers
  useEffect(() => {
    const unlistenMinimize = listen("minimize-window-requested", async () => {
      console.log("Minimize window requested from menu");
      try {
        const window = getCurrentWindow();
        await window.minimize();
        console.log("✅ Window minimized successfully");
      } catch (error) {
        console.error("❌ Failed to minimize window:", error);
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `Failed to minimize window: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      }
    });

    const unlistenZoom = listen("zoom-window-requested", async () => {
      console.log("Zoom window requested from menu");
      try {
        const window = getCurrentWindow();
        const isMaximized = await window.isMaximized();
        if (isMaximized) {
          await window.unmaximize();
          console.log("✅ Window unmaximized successfully");
        } else {
          await window.maximize();
          console.log("✅ Window maximized successfully");
        }
      } catch (error) {
        console.error("❌ Failed to toggle window zoom:", error);
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `Failed to toggle window zoom: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      }
    });

    const unlistenFullscreen = listen(
      "toggle-fullscreen-requested",
      async () => {
        console.log("Toggle fullscreen requested from menu");
        try {
          const window = getCurrentWindow();
          const isFullscreen = await window.isFullscreen();
          await window.setFullscreen(!isFullscreen);
          console.log(
            `✅ Window fullscreen ${
              !isFullscreen ? "enabled" : "disabled"
            } successfully`
          );
        } catch (error) {
          console.error("❌ Failed to toggle fullscreen:", error);
          setConversation((prev) => [
            ...prev,
            {
              role: "system",
              content: `Failed to toggle fullscreen: ${error}`,
              timestamp: Date.now(),
            },
          ]);
        }
      }
    );

    const unlistenUpdate = listen("update-check-requested", () => {
      console.log("Update check requested from menu");
      handleUpdateCheck();
    });

    return () => {
      unlistenMinimize.then((unlistenFn) => unlistenFn());
      unlistenZoom.then((unlistenFn) => unlistenFn());
      unlistenFullscreen.then((unlistenFn) => unlistenFn());
      unlistenUpdate.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Update check implementation - simplified version using backend
  const handleUpdateCheck = async () => {
    setIsCheckingUpdate(true);
    try {
      console.log("Checking for updates...");
      // TODO: Implement actual update check
      setTimeout(() => {
        setIsCheckingUpdate(false);
        console.log("You're running the latest version!");
      }, 2000);
    } catch (error) {
      console.error("Failed to check for updates:", error);
      setIsCheckingUpdate(false);
      console.error("Failed to check for updates");
    }
  };

  const handleSaveResponse = async (messageId: string, content: string) => {
    setSavingMessageId(messageId);
    try {
      // Create a blob with the content
      const blob = new Blob([content], { type: "text/plain" });
      const url = URL.createObjectURL(blob);

      // Create a temporary download link
      const link = document.createElement("a");
      link.href = url;
      link.download = `juno-response-${Date.now()}.txt`;
      document.body.appendChild(link);
      link.click();

      // Clean up
      document.body.removeChild(link);
      URL.revokeObjectURL(url);

      // Clear saving state after a brief delay
      setTimeout(() => setSavingMessageId(null), 1000);
    } catch (error) {
      console.error("Failed to save response:", error);
      setSavingMessageId(null);
    }
  };

  // Modal rendering function
  const renderModal = () => {
    return null; // No modals implemented yet
  };

  // Form submission handler
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim() || isProcessing) return;

    // Add user message to conversation
    const userMessage: ChatMessage = {
      role: "user",
      content: query.trim(),
      timestamp: Date.now(),
    };
    setConversation((prev) => [...prev, userMessage]);
    setQuery("");
    setIsProcessing(true);

    // TODO: Implement actual API call
    setTimeout(() => {
      const assistantMessage: ChatMessage = {
        role: "assistant",
        content:
          "This is a placeholder response. The actual implementation would call the Tauri backend.",
        timestamp: Date.now(),
      };
      setConversation((prev) => [...prev, assistantMessage]);
      setIsProcessing(false);
    }, 1000);
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
                <ModularSettingsWindow />
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
                    setCurrentView("chat");
                  }}
                  onSkip={() => {
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
                                                `copy-${index}`,
                                                msg.content
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
                                                `save-html-${index}`,
                                                msg.content
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
                                                `save-markdown-${index}`,
                                                msg.content
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
