import { AgentExecutionProgressIndicator } from "@/components/AgentExecutionProgressIndicator"; // Import the AgentExecutionProgressIndicator component
import DevToolsPanel from "@/components/DevToolsPanel";
import { PermissionsFlow } from "@/components/PermissionsFlow";
import { ThinkingMessage } from "@/components/ThinkingMessage";
import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { JsxMessageRenderer, isJsxContent } from "@/components/ui/jsx-message-renderer";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import { useSound, useVoiceSounds } from "@/hooks/useSound";
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  DogIcon,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Send,
  Server,
  Trash2,
} from "lucide-react";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { toggleDictation } from "tauri-plugin-voice-transcription-api";
import { FloatingBar } from "./Bar";
import ClickVisualizer from "./components/ClickVisualizer";
import Settings from "./components/Settings";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import "./styles/globals.css";
import { getCurrentWindow } from '@tauri-apps/api/window';

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

  // Enhanced modal and feature state
  const [activeModal, setActiveModal] = useState<ModalType>(null);
  const [feedbackData, setFeedbackData] = useState<FeedbackData>({
    type: "general",
    title: "",
    description: "",
    email: "",
    priority: "medium"
  });
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

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
      setFeedbackData(prev => ({
        ...prev,
        type: feedbackType === "issue" ? "issue" : "general"
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
        setConversation(prev => [...prev, {
          role: "system",
          content: `Failed to minimize window: ${error}`,
          timestamp: Date.now()
        }]);
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
        setConversation(prev => [...prev, {
          role: "system",
          content: `Failed to toggle window zoom: ${error}`,
          timestamp: Date.now()
        }]);
      }
    });

    const unlistenFullscreen = listen("toggle-fullscreen-requested", async () => {
      console.log("Toggle fullscreen requested from menu");
      try {
        const window = getCurrentWindow();
        const isFullscreen = await window.isFullscreen();
        await window.setFullscreen(!isFullscreen);
        console.log(`✅ Window fullscreen ${!isFullscreen ? 'enabled' : 'disabled'} successfully`);
      } catch (error) {
        console.error("❌ Failed to toggle fullscreen:", error);
        setConversation(prev => [...prev, {
          role: "system",
          content: `Failed to toggle fullscreen: ${error}`,
          timestamp: Date.now()
        }]);
      }
    });

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
      console.log("🔍 Checking for updates...");
      
      // Use backend command to check for updates
      const updateResult = await invoke("check_for_updates") as { available: boolean; version?: string; notes?: string };
      
      if (updateResult.available) {
        const updateInfo: UpdateInfo = {
          available: true,
          version: updateResult.version,
          notes: updateResult.notes
        };
        setUpdateInfo(updateInfo);
        setActiveModal("update");
        console.log("✅ Update available:", updateInfo);
      } else {
        // Show "no updates" message in chat
        setConversation(prev => [...prev, {
          role: "system",
          content: "✅ You're running the latest version of Juno AI.",
          timestamp: Date.now()
        }]);
        console.log("✅ No updates available");
      }
    } catch (error) {
      console.error("❌ Failed to check for updates:", error);
      setConversation(prev => [...prev, {
        role: "system",
        content: `Failed to check for updates: ${error}`,
        timestamp: Date.now()
      }]);
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  // Install update implementation - simplified version using backend
  const handleInstallUpdate = async () => {
    try {
      console.log("🚀 Installing update...");
      setConversation(prev => [...prev, {
        role: "system",
        content: "🚀 Installing update... The application will restart automatically.",
        timestamp: Date.now()
      }]);
      
      await invoke("install_update");
    } catch (error) {
      console.error("❌ Failed to install update:", error);
      setConversation(prev => [...prev, {
        role: "system",
        content: `Failed to install update: ${error}`,
        timestamp: Date.now()
      }]);
    }
  };

  // Chat export implementation - simplified version using backend save dialog
  const handleExportChat = async () => {
    if (conversation.length === 0) {
      setConversation(prev => [...prev, {
        role: "system",
        content: "No conversation to export.",
        timestamp: Date.now()
      }]);
      return;
    }

    setIsExporting(true);
    try {
      const exportData: ChatExport = {
        version: "1.0",
        exported_at: new Date().toISOString(),
        conversation: conversation.filter(msg => msg.role !== "system"), // Exclude system messages
        metadata: {
          total_messages: conversation.length,
          export_type: "filtered"
        }
      };

      // Use backend command to handle file save dialog and writing
      const result = await invoke("save_chat_export", { 
        data: JSON.stringify(exportData, null, 2)
      }) as { success: boolean; path?: string; error?: string };
      
      if (result.success && result.path) {
        setConversation(prev => [...prev, {
          role: "system",
          content: `✅ Chat exported successfully to: ${result.path}`,
          timestamp: Date.now()
        }]);
        console.log("✅ Chat exported successfully to:", result.path);
      } else {
        throw new Error(result.error || "Export failed");
      }
    } catch (error) {
      console.error("❌ Failed to export chat:", error);
      setConversation(prev => [...prev, {
        role: "system",
        content: `Failed to export chat: ${error}`,
        timestamp: Date.now()
      }]);
    } finally {
      setIsExporting(false);
      setActiveModal(null);
    }
  };

  // Chat import implementation - simplified version using backend open dialog
  const handleImportChat = async () => {
    setIsImporting(true);
    try {
      // Use backend command to handle file open dialog and reading
      const result = await invoke("load_chat_import") as { 
        success: boolean; 
        data?: string; 
        error?: string;
        messageCount?: number;
      };

      if (result.success && result.data) {
        const importData: ChatExport = JSON.parse(result.data);
        
        // Validate import format
        if (!importData.conversation || !Array.isArray(importData.conversation)) {
          throw new Error("Invalid chat export format");
        }

        // Confirm import with user
        const confirmImport = window.confirm(
          `Import ${result.messageCount || importData.conversation.length} messages? This will replace your current conversation.`
        );

        if (confirmImport) {
          // Add timestamps to imported messages if missing
          const importedMessages = importData.conversation.map(msg => ({
            ...msg,
            timestamp: msg.timestamp || Date.now()
          }));

          setConversation(importedMessages);
          
          setConversation(prev => [...prev, {
            role: "system",
            content: `✅ Chat imported successfully. Loaded ${importedMessages.length} messages.`,
            timestamp: Date.now()
          }]);
          console.log("✅ Chat imported successfully:", importData);
        }
      } else {
        if (result.error && !result.error.includes("cancelled")) {
          throw new Error(result.error);
        }
        // User cancelled - no error needed
      }
    } catch (error) {
      console.error("❌ Failed to import chat:", error);
      setConversation(prev => [...prev, {
        role: "system",
        content: `Failed to import chat: ${error}`,
        timestamp: Date.now()
      }]);
    } finally {
      setIsImporting(false);
      setActiveModal(null);
    }
  };

  // Feedback submission implementation
  const handleSubmitFeedback = async () => {
    if (!feedbackData.title.trim() || !feedbackData.description.trim()) {
      alert("Please fill in both title and description fields.");
      return;
    }

    try {
      console.log("📝 Submitting feedback:", feedbackData);
      
      // Create GitHub issue URL or mailto link for feedback
      if (feedbackData.type === "issue") {
        const title = encodeURIComponent(feedbackData.title);
        const body = encodeURIComponent(
          `**Priority:** ${feedbackData.priority}\n\n**Description:**\n${feedbackData.description}\n\n**Contact:** ${feedbackData.email || 'Not provided'}`
        );
        const githubUrl = `https://github.com/lacymorrow/juno/issues/new?title=${title}&body=${body}`;
        
        // Open GitHub issues page
        await invoke("open_url", { url: githubUrl });
      } else {
        // For general feedback, create mailto link
        const subject = encodeURIComponent(`Juno AI Feedback: ${feedbackData.title}`);
        const body = encodeURIComponent(
          `Priority: ${feedbackData.priority}\n\nDescription:\n${feedbackData.description}`
        );
        const mailtoUrl = `mailto:feedback@juno-ai.com?subject=${subject}&body=${body}`;
        
        await invoke("open_url", { url: mailtoUrl });
      }

      setConversation(prev => [...prev, {
        role: "system",
        content: "✅ Feedback form opened. Thank you for your input!",
        timestamp: Date.now()
      }]);

      // Reset form and close modal
      setFeedbackData({
        type: "general",
        title: "",
        description: "",
        email: "",
        priority: "medium"
      });
      setActiveModal(null);
    } catch (error) {
      console.error("❌ Failed to submit feedback:", error);
      setConversation(prev => [...prev, {
        role: "system",
        content: `Failed to open feedback form: ${error}`,
        timestamp: Date.now()
      }]);
    }
  };

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

  // Listen for TTS audio ready events
  useEffect(() => {
    const unlisten = listen<{ audio_base64: string }>(
      "tts-audio-ready",
      (event) => {
        console.log("TTS audio ready event received");
        const { audio_base64 } = event.payload;
        if (audio_base64) {
          playAudioFromBase64(audio_base64);
        }
      }
    );

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
                isJsx: isJsxContent(complete_text), // Auto-detect JSX in completed stream
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

  // Router logic based on URL path
  const [currentPath, setCurrentPath] = useState(window.location.pathname);

  useEffect(() => {
    const handleLocationChange = () => {
      setCurrentPath(window.location.pathname);
    };

    window.addEventListener("popstate", handleLocationChange);
    return () => window.removeEventListener("popstate", handleLocationChange);
  }, []);

  // If this is the floating bar, render only the floating bar
  if (currentPath === "/floating-bar") {
    return <FloatingBar />;
  }

  // Function to handle example prompt selection
  const handleExamplePromptSelect = useCallback((prompt: string) => {
    setQuery(prompt);
    // Auto-submit the selected prompt
    setTimeout(() => {
      const syntheticEvent = {
        preventDefault: () => {},
      } as React.FormEvent<HTMLFormElement>;
      handleSubmit(syntheticEvent);
    }, 100); // Small delay to ensure state is updated
  }, [handleSubmit]);

  // Enhanced render with new modals
  const renderModal = () => {
    if (!activeModal) return null;

    const modalContent = () => {
      switch (activeModal) {
        case "help":
          return (
            <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-2xl max-h-[80vh] overflow-y-auto">
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-gray-900 dark:text-white">Help & Documentation</h2>
                <button
                  onClick={() => setActiveModal(null)}
                  className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="space-y-4 text-gray-700 dark:text-gray-300">
                <section>
                  <h3 className="text-lg font-semibold mb-2">🎙️ Voice Controls</h3>
                  <ul className="list-disc list-inside space-y-1">
                    <li><strong>Option + D:</strong> Toggle Agent Mode (AI conversations)</li>
                    <li><strong>Option + Space:</strong> Toggle Dictation Mode (voice typing)</li>
                    <li><strong>Wake Words:</strong> Say "Hey Juno" or "Computer" (Always Listening Mode)</li>
                  </ul>
                </section>
                <section>
                  <h3 className="text-lg font-semibold mb-2">💬 Chat Features</h3>
                  <ul className="list-disc list-inside space-y-1">
                    <li>Type your questions and press Enter</li>
                    <li>Use voice commands for hands-free interaction</li>
                    <li>Export conversations for backup or sharing</li>
                    <li>Import previous conversations to continue</li>
                  </ul>
                </section>
                <section>
                  <h3 className="text-lg font-semibold mb-2">🛠️ Tools & Automation</h3>
                  <ul className="list-disc list-inside space-y-1">
                    <li>Screen capture and analysis</li>
                    <li>File operations and code analysis</li>
                    <li>Web browsing automation</li>
                    <li>System control and monitoring</li>
                  </ul>
                </section>
                <section>
                  <h3 className="text-lg font-semibold mb-2">⚙️ Settings & Permissions</h3>
                  <ul className="list-disc list-inside space-y-1">
                    <li>Configure accessibility permissions for screen control</li>
                    <li>Adjust voice recognition settings</li>
                    <li>Customize keyboard shortcuts</li>
                    <li>Enable developer tools for advanced features</li>
                  </ul>
                </section>
                <div className="pt-4 border-t border-gray-200 dark:border-gray-600">
                  <p className="text-sm text-gray-600 dark:text-gray-400">
                    For more detailed documentation, visit our GitHub repository or contact support.
                  </p>
                </div>
              </div>
            </div>
          );

        case "feedback":
          return (
            <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-gray-900 dark:text-white">Submit Feedback</h2>
                <button
                  onClick={() => setActiveModal(null)}
                  className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <form onSubmit={(e) => { e.preventDefault(); handleSubmitFeedback(); }} className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Feedback Type
                  </label>
                  <select
                    value={feedbackData.type}
                    onChange={(e) => setFeedbackData(prev => ({ ...prev, type: e.target.value as "issue" | "feature" | "general" }))}
                    className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                  >
                    <option value="general">General Feedback</option>
                    <option value="issue">Bug Report</option>
                    <option value="feature">Feature Request</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Title *
                  </label>
                  <input
                    type="text"
                    value={feedbackData.title}
                    onChange={(e) => setFeedbackData(prev => ({ ...prev, title: e.target.value }))}
                    className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                    placeholder="Brief summary of your feedback"
                    required
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Description *
                  </label>
                  <textarea
                    value={feedbackData.description}
                    onChange={(e) => setFeedbackData(prev => ({ ...prev, description: e.target.value }))}
                    className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white h-24"
                    placeholder="Detailed description of your feedback"
                    required
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Priority
                  </label>
                  <select
                    value={feedbackData.priority}
                    onChange={(e) => setFeedbackData(prev => ({ ...prev, priority: e.target.value as "low" | "medium" | "high" }))}
                    className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                  >
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Email (Optional)
                  </label>
                  <input
                    type="email"
                    value={feedbackData.email}
                    onChange={(e) => setFeedbackData(prev => ({ ...prev, email: e.target.value }))}
                    className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                    placeholder="your.email@example.com"
                  />
                </div>
                <div className="flex gap-3 pt-2">
                  <button
                    type="button"
                    onClick={() => setActiveModal(null)}
                    className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600"
                  >
                    Submit
                  </button>
                </div>
              </form>
            </div>
          );

        case "export":
          return (
            <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-gray-900 dark:text-white">Export Chat</h2>
                <button
                  onClick={() => setActiveModal(null)}
                  className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="space-y-4">
                <p className="text-gray-700 dark:text-gray-300">
                  Export your current conversation to a JSON file for backup or sharing.
                </p>
                <div className="bg-gray-50 dark:bg-gray-700 p-3 rounded text-sm">
                  <strong>Messages to export:</strong> {conversation.filter(msg => msg.role !== "system").length}<br />
                  <strong>Format:</strong> JSON<br />
                  <strong>Includes:</strong> All user and assistant messages
                </div>
                <div className="flex gap-3 pt-2">
                  <button
                    onClick={() => setActiveModal(null)}
                    className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleExportChat}
                    disabled={isExporting}
                    className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600 disabled:opacity-50"
                  >
                    {isExporting ? "Exporting..." : "Export"}
                  </button>
                </div>
              </div>
            </div>
          );

        case "import":
          return (
            <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-gray-900 dark:text-white">Import Chat</h2>
                <button
                  onClick={() => setActiveModal(null)}
                  className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="space-y-4">
                <p className="text-gray-700 dark:text-gray-300">
                  Import a previously exported chat conversation from a JSON file.
                </p>
                <div className="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 p-3 rounded text-sm text-yellow-800 dark:text-yellow-200">
                  <strong>Warning:</strong> This will replace your current conversation. Make sure to export it first if you want to keep it.
                </div>
                <div className="flex gap-3 pt-2">
                  <button
                    onClick={() => setActiveModal(null)}
                    className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleImportChat}
                    disabled={isImporting}
                    className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50"
                  >
                    {isImporting ? "Importing..." : "Select File"}
                  </button>
                </div>
              </div>
            </div>
          );

        case "update":
          return updateInfo ? (
            <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-gray-900 dark:text-white">Update Available</h2>
                <button
                  onClick={() => setActiveModal(null)}
                  className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="space-y-4">
                <div className="space-y-2">
                  <p className="text-gray-700 dark:text-gray-300">
                    A new version of Juno AI is available!
                  </p>
                  {updateInfo.version && (
                    <div className="bg-blue-50 dark:bg-blue-900/20 p-3 rounded">
                      <p className="text-sm"><strong>Version:</strong> {updateInfo.version}</p>
                      {updateInfo.date && <p className="text-sm"><strong>Date:</strong> {updateInfo.date}</p>}
                    </div>
                  )}
                  {updateInfo.notes && (
                    <div className="bg-gray-50 dark:bg-gray-700 p-3 rounded">
                      <p className="text-sm font-medium mb-1">Release Notes:</p>
                      <p className="text-sm text-gray-600 dark:text-gray-400">{updateInfo.notes}</p>
                    </div>
                  )}
                </div>
                <div className="flex gap-3 pt-2">
                  <button
                    onClick={() => setActiveModal(null)}
                    className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                  >
                    Later
                  </button>
                  <button
                    onClick={handleInstallUpdate}
                    className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600"
                  >
                    Install Update
                  </button>
                </div>
              </div>
            </div>
          ) : null;

        default:
          return null;
      }
    };

    return (
      <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        {modalContent()}
      </div>
    );
  };

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
                <div className="flex items-center gap-4 text-sm text-muted-foreground">
                  <div className="flex items-center gap-2">
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
                  {currentView === "chat" && serverStatus === "connected" && (
                    <div className="border-l pl-4">
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
                            <p className="text-muted-foreground mb-6">
                              Your intelligent desktop companion with advanced
                              voice capabilities
                            </p>
                          </div>
                        </div>
                        
                        {/* Example Prompts */}
                        <ExamplePrompts onPromptSelect={handleExamplePromptSelect} />
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
                                  {msg.role === "assistant" &&
                                  (!msg.content ||
                                    msg.content.trim() === "") ? (
                                    <span className="text-muted-foreground italic flex items-center gap-2">
                                      <span>✓</span>
                                      <span>Task completed successfully</span>
                                    </span>
                                  ) : msg.isJsx || (msg.role === "assistant" && isJsxContent(msg.content)) ? (
                                    <JsxMessageRenderer jsx={msg.content} />
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
      
      {/* Enhanced modal system */}
      {renderModal()}
      
      {/* Update check loading indicator */}
      {isCheckingUpdate && (
        <div className="fixed bottom-4 right-4 bg-blue-500 text-white px-4 py-2 rounded-lg shadow-lg">
          Checking for updates...
        </div>
      )}
    </main>
  );
}

export default App;
