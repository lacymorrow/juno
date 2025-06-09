import ClickVisualizer from "@/components/ClickVisualizer"; // Import the ClickVisualizer
import DevToolsPanel from "@/components/DevToolsPanel"; // Import the new panel
import { PermissionsFlow } from "@/components/PermissionsFlow"; // Import the PermissionsFlow component
import Settings from "@/components/Settings"; // Import the Settings component
import { ThinkingMessage } from "@/components/ThinkingMessage"; // Import the ThinkingMessage component
import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage"; // Import the ToolCall components
import { Button } from "@/components/ui/button"; // Shadcn Button
import { Input } from "@/components/ui/input"; // Shadcn Input
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"; // Import Resizable components
import { ScrollArea } from "@/components/ui/scroll-area"; // Import Shadcn ScrollArea
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator"; // Import the VoiceStatusIndicator component
import { useSound, useVoiceSounds } from "@/hooks/useSound"; // Import sound hooks
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService"; // Import TTS service
import { cn } from "@/lib/utils"; // Shadcn utility
import { invoke } from "@tauri-apps/api/core"; // Use Tauri's invoke
import { listen } from "@tauri-apps/api/event"; // Import listen
import {
  ArrowLeft,
  Brain,
  DogIcon,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Send,
  Server,
  Trash2,
  Type,
} from "lucide-react"; // Icons
import { useCallback, useEffect, useRef, useState } from "react";
import { toggleDictation } from "tauri-plugin-voice-transcription-api"; // Import toggleDictation from plugin API

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
type AppView = "chat" | "settings" | "devtools" | "permissions";

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

function App() {
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus, setServerStatus] = useState<
    "checking" | "connected" | "error"
  >("checking");
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false); // State for collapsible panel
  const [currentView, setCurrentView] = useState<AppView>("chat"); // State for current view
  const conversationEndRef = useRef<HTMLDivElement>(null);
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(
    null
  );

  // Sound hooks
  const sound = useSound();
  const voiceSounds = useVoiceSounds();

  // Permissions state
  const [, setShowPermissionsFlow] = useState(false);
  const [, setPermissionsChecked] = useState(false);

  // Check permissions on startup
  useEffect(() => {
    const checkInitialPermissions = async () => {
      try {
        const result = await invoke<{
          accessibility: { granted: boolean; required: boolean };
          screenRecording: { granted: boolean; required: boolean };
          microphone: { granted: boolean; required: boolean };
          allGranted: boolean;
        }>("check_permissions_status");

        setPermissionsChecked(true);

        // Show permissions flow if any required permissions are missing
        if (!result.allGranted) {
          setShowPermissionsFlow(true);
          setCurrentView("permissions");
        }
      } catch (error) {
        console.error("Error checking permissions:", error);
        setPermissionsChecked(true);
        // Optionally show permissions flow even on error
        setShowPermissionsFlow(true);
        setCurrentView("permissions");
      }
    };

    checkInitialPermissions();
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

      // If we reach here for dictation:
      // - text is not empty.
      // - serverStatus check was skipped (or passed if not dictation).
      // - isProcessing check was skipped (or passed if not dictation).

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

  // Listen for settings menu requests from native menu
  useEffect(() => {
    const unlisten = listen<string>("settings-requested", (event) => {
      console.log("Settings requested from menu:", event.payload);
      setCurrentView("settings");
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

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

  // Listen for help menu requests
  useEffect(() => {
    const unlisten = listen<string>("help-requested", (event) => {
      console.log("Help requested from menu:", event.payload);
      const helpType = event.payload;

      if (helpType === "shortcuts") {
        // Show keyboard shortcuts - could navigate to settings or show modal
        setCurrentView("settings");
      } else {
        // General help - could open documentation or show help modal
        console.log("General help requested");
        // TODO: Implement help modal or navigate to help section
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

  // Listen for feedback requests
  useEffect(() => {
    const unlisten = listen<string>("feedback-requested", (event) => {
      console.log("Feedback requested from menu:", event.payload);
      const feedbackType = event.payload;

      // TODO: Implement feedback modal or form
      if (feedbackType === "issue") {
        console.log("Issue report requested");
        // Could open GitHub issues page or feedback form
      } else {
        console.log("General feedback requested");
        // Could open feedback form
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for import/export chat requests
  useEffect(() => {
    const unlistenImport = listen("import-chat-requested", () => {
      console.log("Import chat requested from menu");
      // TODO: Implement chat import functionality
    });

    const unlistenExport = listen("export-chat-requested", () => {
      console.log("Export chat requested from menu");
      // TODO: Implement chat export functionality
    });

    return () => {
      unlistenImport.then((unlistenFn) => unlistenFn());
      unlistenExport.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for window management requests
  useEffect(() => {
    const unlistenMinimize = listen("minimize-window-requested", () => {
      console.log("Minimize window requested from menu");
      // TODO: Implement window minimize
    });

    const unlistenZoom = listen("zoom-window-requested", () => {
      console.log("Zoom window requested from menu");
      // TODO: Implement window zoom
    });

    const unlistenFullscreen = listen("toggle-fullscreen-requested", () => {
      console.log("Toggle fullscreen requested from menu");
      // TODO: Implement fullscreen toggle
    });

    const unlistenUpdate = listen("update-check-requested", () => {
      console.log("Update check requested from menu");
      // TODO: Implement update check functionality
    });

    return () => {
      unlistenMinimize.then((unlistenFn) => unlistenFn());
      unlistenZoom.then((unlistenFn) => unlistenFn());
      unlistenFullscreen.then((unlistenFn) => unlistenFn());
      unlistenUpdate.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for transcription results from dictation
  useEffect(() => {
    const unlisten = listen<{ query?: string | null; error?: string | null }>( // Define the expected payload structure
      "app-dictation-finished",
      (event) => {
        // Listen for "app-dictation-finished"
        console.log("Received app-dictation-finished event:", event.payload);
        const transcribedText = event.payload?.query; // Extract text from payload.query
        const error = event.payload?.error;

        if (error) {
          console.error("Dictation error:", error);
          // Voice error sound is now played by the backend when transcription fails
          // Optionally, display this error to the user in the chat or via a notification
          setConversation((prev) => [
            ...prev,
            {
              role: "system",
              content: `Dictation failed: ${error}`,
              timestamp: Date.now(),
            },
          ]);
          return; // Stop further processing if there was an error
        }

        if (transcribedText && transcribedText.trim() !== "") {
          // Only play sound for successful Agent Mode transcription (not Dictation Mode)
          console.log(
            "Transcribed text received, automatically submitting to AI agent:",
            transcribedText
          );
          submitQuery(transcribedText, true); // isFromDictation = true
        } else {
          console.log(
            "Received empty, whitespace-only, or null transcription, not submitting."
          );
          // Note: No need to play notification sound here - let backend handle feedback
        }
      }
    );

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [submitQuery, voiceSounds, sound]);

  // Listen for dictation toggle requests
  useEffect(() => {
    const unlisten = listen("toggle-dictation-request", async () => {
      console.log("Received toggle-dictation-request event");
      try {
        const isNowDictating = await toggleDictation();
        console.log("Toggled dictation, now dictating:", isNowDictating);

        // Voice start/end sounds are now played by the backend automatically
      } catch (error) {
        console.error("Failed to toggle dictation:", error);
        // Error sound for failed toggle is now played by the backend
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `Failed to toggle dictation: ${error}`,
          },
        ]);
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [voiceSounds, sound]);

  // Check server status on mount
  useEffect(() => {
    const checkServer = async () => {
      try {
        const isConnected: boolean = await invoke("check_server_status");
        if (isConnected) {
          setServerStatus("connected");
          setConversation([
            {
              role: "system",
              content: "Connected. Enter your query below.",
            },
          ]);
        } else {
          setServerStatus("error");
          setConversation([
            {
              role: "system",
              content: "Failed to connect to backend. Please check logs.",
            },
          ]);
        }
      } catch (error) {
        setServerStatus("error");
        setConversation([
          {
            role: "system",
            content: `Error connecting to backend: ${error}. Check console logs.`,
          },
        ]);
      }
    };
    checkServer();
  }, []);

  // Listen for responses broadcast from the backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<BackendResponsePayload>(
        "backend-response",
        (event) => {
          console.log("Received backend-response event (raw):", event.payload);
          // Call the debounced handler
          handleBackendResponse(event.payload);
        }
      );
    };

    setupListener();

    // Cleanup listener on component unmount
    return () => {
      unlisten?.();
    };
  }, [handleBackendResponse]); // Add debounced handler to dependency array

  // Listen for agent stopping events to stop TTS
  useEffect(() => {
    const unlisten = listen("agent-stopping", async () => {
      console.log("Agent stopping event received - stopping TTS");
      try {
        await stopTTS((msg, level) =>
          console.log(`[TTS-${level || "info"}] ${msg}`)
        );
      } catch (error) {
        console.error("Error stopping TTS:", error);
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Listen for agent events (thinking, tool calls, etc.)
  useEffect(() => {
    const unlistenPromise = listen<AgentEventTauri>("agent-event", (event) => {
      const { type, payload } = event.payload;
      const currentTime = Date.now();

      setConversation((prev) => {
        let newMessage: ChatMessage | null = null;

        if (type === "thinking" && "content" in payload) {
          newMessage = {
            role: "thinking",
            content: payload.content || "Thinking...",
            timestamp: currentTime,
          };
        } else if (type === "tool_call_request" && "tool_name" in payload) {
          const requestPayload = payload as ToolCallRequestPayload;
          newMessage = {
            role: "tool_call_request",
            tool_name: requestPayload.tool_name,
            tool_args: requestPayload.tool_args,
            content:
              requestPayload.content ||
              `Using tool: ${requestPayload.tool_name}`,
            timestamp: currentTime,
          };
        } else if (type === "tool_call_result" && "tool_name" in payload) {
          const resultPayload = payload as ToolCallResultPayload;
          newMessage = {
            role: "tool_call_result",
            tool_name: resultPayload.tool_name,
            tool_output: resultPayload.tool_output,
            success: resultPayload.success,
            content:
              resultPayload.content ||
              (resultPayload.success
                ? `Tool ${resultPayload.tool_name} executed successfully.`
                : `Tool ${resultPayload.tool_name} failed.`),
            screenshot_base64: resultPayload.screenshot_base64,
            timestamp: currentTime,
          };
        } else if (type === "generic_content" && "content" in payload) {
          newMessage = {
            role: "system",
            content: payload.content || "System message",
            timestamp: currentTime,
          };
        }

        if (newMessage) {
          return [...prev, newMessage];
        } else {
          return prev;
        }
      });
    });

    return () => {
      unlistenPromise.then((unlistenFn) => unlistenFn());
    };
  }, []); // Empty dependency array, so it runs once on mount and cleans up on unmount

  // Listen for streaming events
  useEffect(() => {
    const streamStartListener = listen<StreamStartEvent>(
      "agent-stream-start",
      (event) => {
        console.log("Stream started:", event.payload);
        const { message_id } = event.payload;

        // Create a new streaming assistant message
        const streamingMessage: ChatMessage = {
          role: "assistant",
          content: "",
          timestamp: Date.now(),
          isStreaming: true,
          messageId: message_id,
        };

        setConversation((prev) => [...prev, streamingMessage]);
      }
    );

    const streamTextListener = listen<StreamingTextEvent>(
      "agent-text-stream",
      (event) => {
        console.log("Stream text chunk:", event.payload);
        const { chunk, message_id } = event.payload;

        // Update the streaming message with the new chunk
        setConversation((prev) =>
          prev.map((msg) => {
            if (msg.messageId === message_id && msg.isStreaming) {
              return {
                ...msg,
                content: msg.content + chunk,
              };
            }
            return msg;
          })
        );

        // Auto-scroll to bottom during streaming
        setTimeout(() => {
          conversationEndRef.current?.scrollIntoView({ behavior: "smooth" });
        }, 50);
      }
    );

    const streamEndListener = listen<StreamEndEvent>(
      "agent-stream-end",
      (event) => {
        console.log("Stream ended:", event.payload);
        const { message_id, complete_text } = event.payload;

        // Finalize the streaming message
        setConversation((prev) =>
          prev.map((msg) => {
            if (msg.messageId === message_id && msg.isStreaming) {
              return {
                ...msg,
                content: complete_text,
                isStreaming: false,
              };
            }
            return msg;
          })
        );

        // Reset processing state since the AI has finished responding
        setIsProcessing(false);
      }
    );

    return () => {
      streamStartListener.then((unlistenFn) => unlistenFn());
      streamTextListener.then((unlistenFn) => unlistenFn());
      streamEndListener.then((unlistenFn) => unlistenFn());
    };
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitQuery(query);
  };

  // Helper function to convert base64 to Blob
  function base64ToBlob(base64: string, contentType = "audio/mpeg"): Blob {
    const byteCharacters = atob(base64);
    const byteNumbers = new Array(byteCharacters.length);
    for (let i = 0; i < byteCharacters.length; i++) {
      byteNumbers[i] = byteCharacters.charCodeAt(i);
    }
    const byteArray = new Uint8Array(byteNumbers);
    return new Blob([byteArray], { type: contentType });
  }

  // Helper function to play audio from base64 data
  const playAudioFromBase64 = (base64Audio: string) => {
    // Stop any currently playing audio
    if (currentAudio) {
      currentAudio.pause();
      currentAudio.currentTime = 0;
      if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
        URL.revokeObjectURL(currentAudio.src);
      }
      currentAudio.src = ""; // Clear the source
    }

    try {
      const audioBlob = base64ToBlob(base64Audio);
      const audioUrl = URL.createObjectURL(audioBlob);
      const newAudio = new Audio(audioUrl);
      setCurrentAudio(newAudio); // Store the new audio element
      setCurrentAudioElement(newAudio); // Sync with TTS service

      newAudio.play();

      newAudio.onended = () => {
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
        setCurrentAudioElement(null); // Sync with TTS service

        // Notify backend that TTS has finished so it can play the success sound
        invoke("handle_tts_completion").catch((error) => {
          console.error("Failed to notify backend of TTS completion:", error);
        });
      };
      newAudio.onerror = (e) => {
        console.error("Audio playback error:", e);
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
        setCurrentAudioElement(null); // Sync with TTS service
      };
    } catch (error) {
      console.error("Error processing or playing audio:", error);
      setCurrentAudio(null);
      setCurrentAudioElement(null); // Sync with TTS service
    }
  };

  // Cleanup effect for audio
  useEffect(() => {
    return () => {
      if (currentAudio) {
        currentAudio.pause();
        currentAudio.currentTime = 0; // Reset playback position
        if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
          URL.revokeObjectURL(currentAudio.src);
        }
        setCurrentAudio(null); // Clear the audio reference
        setCurrentAudioElement(null); // Sync with TTS service
      }
    };
  }, [currentAudio]);

  // Scroll conversation to bottom
  useEffect(() => {
    conversationEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [conversation]);

  return (
    <main className="h-screen flex flex-col">
      {/* Click Visualizer - overlays the entire app to show click indicators (from tools2) */}
      <ClickVisualizer />

      <div className="w-screen h-screen bg-background text-foreground">
        <div className="container mx-auto p-4 h-full flex flex-col">
          {/* Header */}
          <header className="flex justify-between items-center mb-4 p-4 border-b">
            <div className="flex items-center gap-3">
              <DogIcon size={32} className="text-blue-500" />
              <div>
                <h1 className="text-xl font-bold">
                  {currentView === "settings"
                    ? "Settings"
                    : currentView === "devtools"
                    ? "Developer Tools"
                    : currentView === "permissions"
                    ? "Permissions"
                    : "Juno AI Assistant"}
                </h1>
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Server
                    size={14}
                    className={
                      serverStatus === "connected"
                        ? "text-green-500"
                        : serverStatus === "error"
                        ? "text-red-500"
                        : "text-yellow-500"
                    }
                  />
                  <span>
                    {serverStatus === "connected"
                      ? "Connected"
                      : serverStatus === "error"
                      ? "Connection Error"
                      : "Checking..."}
                  </span>
                </div>
              </div>
            </div>

            {/* Voice Status Indicator - only show in chat view */}
            {currentView === "chat" && (
              <div className="flex-1 flex justify-center mx-4">
                <VoiceStatusIndicator
                  variant="compact"
                  className="max-w-md"
                  showText={true}
                />
              </div>
            )}

            <div className="flex items-center gap-2">
              {/* Back Button - show for settings and devtools views */}
              {(currentView === "settings" ||
                currentView === "devtools" ||
                currentView === "permissions") && (
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => setCurrentView("chat")}
                  title="Back to Chat"
                >
                  <ArrowLeft size={18} />
                </Button>
              )}
              {/* Toggle Dev Panel Button - only show in chat view */}
              {currentView === "chat" && (
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => setIsDevPanelOpen(!isDevPanelOpen)}
                  title={isDevPanelOpen ? "Hide Dev Panel" : "Show Dev Panel"}
                >
                  {isDevPanelOpen ? (
                    <PanelLeftClose size={18} />
                  ) : (
                    <PanelLeftOpen size={18} />
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
              <ScrollArea className="h-full w-full p-4">
                <h2 className="text-lg font-semibold mb-3 border-b pb-2">
                  Developer Tools & Logs
                </h2>
                <DevToolsPanel />
              </ScrollArea>
            </div>
          ) : currentView === "permissions" ? (
            <div className="flex-grow rounded-lg border overflow-hidden">
              <ScrollArea className="h-full w-full p-4">
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
                <div className="flex flex-col h-full p-4">
                  {/* Conversation Area */}
                  <ScrollArea className="flex-1 min-h-0 mb-4 -mr-4 pr-4">
                    {conversation.length === 0 ? (
                      /* Welcome message when conversation is empty */
                      <div className="flex flex-col items-center justify-center h-full text-center space-y-6 p-8">
                        <div className="space-y-4">
                          <DogIcon
                            size={64}
                            className="text-blue-500 mx-auto"
                          />
                          <div>
                            <h2 className="text-2xl font-bold mb-2">
                              Welcome to Juno AI Assistant
                            </h2>
                            <p className="text-muted-foreground">
                              Your intelligent desktop companion with advanced
                              voice capabilities
                            </p>
                          </div>
                        </div>

                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-2xl">
                          <div className="p-4 bg-orange-50 dark:bg-orange-950/20 rounded-lg border border-orange-200 dark:border-orange-800">
                            <div className="flex items-center gap-3 mb-2">
                              <Type className="h-5 w-5 text-orange-600 dark:text-orange-400" />
                              <span className="font-semibold text-orange-900 dark:text-orange-100">
                                Quick Dictation
                              </span>
                            </div>
                            <p className="text-sm text-orange-700 dark:text-orange-300 mb-2">
                              Hold{" "}
                              <kbd className="px-1 py-0.5 bg-orange-200 dark:bg-orange-800 rounded text-xs">
                                ⌥+Space
                              </kbd>{" "}
                              to instantly type your speech anywhere
                            </p>
                            <p className="text-xs text-orange-600 dark:text-orange-400">
                              Perfect for emails, documents, and quick text
                              input
                            </p>
                          </div>

                          <div className="p-4 bg-blue-50 dark:bg-blue-950/20 rounded-lg border border-blue-200 dark:border-blue-800">
                            <div className="flex items-center gap-3 mb-2">
                              <Brain className="h-5 w-5 text-blue-600 dark:text-blue-400" />
                              <span className="font-semibold text-blue-900 dark:text-blue-100">
                                AI Conversations
                              </span>
                            </div>
                            <p className="text-sm text-blue-700 dark:text-blue-300 mb-2">
                              Press{" "}
                              <kbd className="px-1 py-0.5 bg-blue-200 dark:bg-blue-800 rounded text-xs">
                                ⌥+D
                              </kbd>{" "}
                              to chat with your AI assistant
                            </p>
                            <p className="text-xs text-blue-600 dark:text-blue-400">
                              Get help with tasks, research, and complex
                              questions
                            </p>
                          </div>
                        </div>

                        <div className="text-xs text-muted-foreground">
                          <p>
                            💡 <strong>Pro tip:</strong> The floating status bar
                            shows real-time voice feedback
                          </p>
                          <p>
                            Use the input field below or try the voice shortcuts
                            to get started
                          </p>
                        </div>
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
                                <span
                                  className={cn(
                                    "inline-block max-w-[85%] px-3 py-1.5 rounded-lg shadow-sm",
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
                                  {msg.role === "assistant" && (!msg.content || msg.content.trim() === "") ? (
                                    <span className="text-muted-foreground italic flex items-center gap-2">
                                      <span>✓</span>
                                      <span>Task completed successfully</span>
                                    </span>
                                  ) : (
                                    msg.content
                                  )}
                                  {msg.screenshot_base64 && (
                                    <div
                                      className={cn(
                                        "mt-2",
                                        msg.role !== "system" && "border-t pt-2"
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
                              </div>
                            )}
                          </div>
                        );
                      })
                    )}
                    <div ref={conversationEndRef} />
                  </ScrollArea>

                  {/* Voice Shortcuts Helper - only show when there's a conversation */}
                  {conversation.length > 0 && (
                    <div className="flex justify-center mb-3">
                      <div className="text-xs text-muted-foreground bg-muted/50 px-3 py-1 rounded-full border">
                        <span className="font-medium">Voice Shortcuts:</span>
                        <span className="mx-2">⌥+D for AI Agent</span>
                        <span className="mx-2">⌥+Space for Dictation</span>
                        <span className="mx-2">Esc to Cancel</span>
                      </div>
                    </div>
                  )}

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
                    <ScrollArea className="h-full w-full p-3">
                      {/* Title (replaces CardHeader) */}
                      <h2 className="text-lg font-semibold mb-3 border-b pb-2">
                        Developer Tools & Logs
                      </h2>
                      {/* DevToolsPanel Component */}
                      <div className="border-b pb-3 mb-3">
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
    </main>
  );
}

export default App;
