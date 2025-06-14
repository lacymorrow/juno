import { AppHeader, type AppView } from "@/components/AppHeader";
import {
  ChatMessageComponent,
  type ChatMessage,
} from "@/components/ChatMessage";
import DevToolsPanel from "@/components/DevToolsPanel";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import {
  ModalSystem,
  type ModalType,
  type FeedbackData,
  type UpdateInfo,
  type ChatExport,
} from "@/components/ModalSystem";
import { PermissionsFlow } from "@/components/PermissionsFlow";
import { isJsxContent } from "@/components/ui/jsx-message-renderer";
import {
  AIInput,
  AIInputButton,
  AIInputSubmit,
  AIInputTextarea,
  AIInputToolbar,
  AIInputTools,
} from "@/components/ui/kibo-ui/ai";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { VoiceProvider } from "@/contexts/VoiceContext";
import { useSound, useVoiceSounds } from "@/hooks/useSound";
import { notificationService } from "@/lib/notifications";
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DogIcon, Plus, Send, Square, Trash2 } from "lucide-react";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { Toaster, toast } from "sonner";
import { toggleDictation } from "tauri-plugin-voice-transcription-api";
import ClickVisualizer from "./components/ClickVisualizer";
import CommandOverlay from "./components/CommandOverlay";
import { FloatingBar } from "./components/FloatingBar";
import KeyPressOverlay from "./components/KeyPressOverlay";
import ToolApprovalModal from "./components/ToolApprovalModal";
import "./styles/globals.css";

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
  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    timeZoneName: "short",
  }).format(new Date(timestamp));
}

// Helper function to render messages using Kibo UI components

function App() {
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [serverConfig, setServerConfig] = useState<any>(null);
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [query, setQuery] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus, setServerStatus] = useState<
    "connected" | "error" | "connecting"
  >("connecting");
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
  const [_permissionsGranted, setPermissionsGranted] = useState(false);

  // Enhanced modal and feature state
  const [activeModal, setActiveModal] = useState<ModalType>(null);
  const [feedbackData, setFeedbackData] = useState<FeedbackData>({
    type: "general",
    title: "",
    description: "",
    email: "",
    priority: "medium",
  });
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  // Copy and save operation state
  const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
  const [savingMessageId, setSavingMessageId] = useState<string | null>(null);

  // Onboarding state
  const [_showOnboarding, _setShowOnboarding] = useState(false);
  const [_onboardingChecked, _setOnboardingChecked] = useState(false);

  // Keyboard shortcuts state
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<{
    agent_mode_toggle: string;
    dictation_input: string;
    stop_current_task: string;
    open_settings: string;
  } | null>(null);

  // Add state for scroll management
  const [userHasScrolledUp, setUserHasScrolledUp] = useState(false);
  const [lastScrollTime, setLastScrollTime] = useState(0);
  const scrollAreaRef = useRef<HTMLDivElement>(null);

  // Add missing view state variables
  const [currentView, setCurrentView] = useState<AppView>("chat");
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false);

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

  // Load keyboard shortcuts
  useEffect(() => {
    const loadShortcuts = async () => {
      try {
        const shortcuts = await invoke<{
          agent_mode_toggle: string;
          dictation_input: string;
          stop_current_task: string;
          open_settings: string;
        }>("get_keyboard_shortcuts");
        setKeyboardShortcuts(shortcuts);
      } catch (error) {
        console.error("Failed to load keyboard shortcuts:", error);
        // Keep defaults if loading fails
      }
    };

    loadShortcuts();
  }, []);

  // Consolidated startup flow - check both permissions and onboarding status
  useEffect(() => {
    const initializeApp = async () => {
      try {
        // Initialize notification service
        await notificationService.initialize();
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

        // Store permissions result for onboarding window
        setPermissionsGranted(permissionsResult.allGranted);

        // Then check onboarding status
        const isDevMode = import.meta.env.DEV;
        const hasCompletedOnboarding = localStorage.getItem(
          "juno-onboarding-completed"
        );

        // Decision logic for which flow to show
        if (isDevMode) {
          // Dev mode: Always show onboarding for QA
          console.log("Dev mode detected - opening onboarding window for QA");
          await invoke("open_onboarding_window");
          setCurrentView("chat");
        } else if (!hasCompletedOnboarding) {
          // First-time user: Show onboarding window
          console.log("First-time user detected - opening onboarding window");
          await invoke("open_onboarding_window");
          setCurrentView("chat");
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
      } catch (error) {
        console.error("Error during app initialization:", error);
        setPermissionsChecked(true);

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

  // Listen for onboarding completion events from the onboarding window
  useEffect(() => {
    const unlistenComplete = listen<{ prompt?: string }>(
      "onboarding-complete",
      async (_event) => {
        console.log("Onboarding completed from separate window");
        // Focus main window
        const currentWindow = getCurrentWindow();
        await currentWindow.show();
        await currentWindow.setFocus();
      }
    );

    const unlistenSkipped = listen<{}>("onboarding-skipped", async (_event) => {
      console.log("Onboarding skipped from separate window");
      // Focus main window
      const currentWindow = getCurrentWindow();
      await currentWindow.show();
      await currentWindow.setFocus();
    });

    return () => {
      unlistenComplete.then((fn) => fn());
      unlistenSkipped.then((fn) => fn());
    };
  }, [submitQuery]);

  // Listen for settings menu requests from native menu
  useEffect(() => {
    const unlisten = listen<string>("settings-requested", async (event) => {
      console.log("Settings requested from menu:", event.payload);
      try {
        await invoke("open_settings_window");
      } catch (error) {
        console.error("Failed to open settings window:", error);
      }
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
    const unlisten = listen<string>("help-requested", async (event) => {
      console.log("Help requested from menu:", event.payload);
      const helpType = event.payload;

      if (helpType === "shortcuts") {
        // Show keyboard shortcuts - open settings window
        try {
          await invoke("open_settings_window");
        } catch (error) {
          console.error("Failed to open settings window:", error);
        }
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
      setFeedbackData((prev) => ({
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
      console.log("🔍 Checking for updates...");

      // Use backend command to check for updates
      const updateResult = (await invoke("check_for_updates")) as {
        available: boolean;
        version?: string;
        notes?: string;
      };

      if (updateResult.available) {
        const updateInfo: UpdateInfo = {
          available: true,
          version: updateResult.version,
          notes: updateResult.notes,
        };
        setUpdateInfo(updateInfo);
        setActiveModal("update");
        console.log("✅ Update available:", updateInfo);
      } else {
        // Show "no updates" message in chat
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: "✅ You're running the latest version of Juno AI.",
            timestamp: Date.now(),
          },
        ]);
        console.log("✅ No updates available");
      }
    } catch (error) {
      console.error("❌ Failed to check for updates:", error);
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Failed to check for updates: ${error}`,
          timestamp: Date.now(),
        },
      ]);
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  // Install update implementation - simplified version using backend
  const handleInstallUpdate = async () => {
    try {
      console.log("🚀 Installing update...");
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content:
            "🚀 Installing update... The application will restart automatically.",
          timestamp: Date.now(),
        },
      ]);

      await invoke("install_update");
    } catch (error) {
      console.error("❌ Failed to install update:", error);
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Failed to install update: ${error}`,
          timestamp: Date.now(),
        },
      ]);
    }
  };

  // Chat export implementation - simplified version using backend save dialog
  const handleExportChat = async () => {
    if (conversation.length === 0) {
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: "No conversation to export.",
          timestamp: Date.now(),
        },
      ]);
      return;
    }

    setIsExporting(true);
    try {
      const exportData: ChatExport = {
        version: "1.0",
        exported_at: new Date().toISOString(),
        conversation: conversation.filter((msg) => msg.role !== "system"), // Exclude system messages
        metadata: {
          total_messages: conversation.length,
          export_type: "filtered",
        },
      };

      // Use backend command to handle file save dialog and writing
      const result = (await invoke("save_chat_export", {
        data: JSON.stringify(exportData, null, 2),
      })) as { success: boolean; path?: string; error?: string };

      if (result.success && result.path) {
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `✅ Chat exported successfully to: ${result.path}`,
            timestamp: Date.now(),
          },
        ]);
        console.log("✅ Chat exported successfully to:", result.path);
      } else {
        throw new Error(result.error || "Export failed");
      }
    } catch (error) {
      console.error("❌ Failed to export chat:", error);
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Failed to export chat: ${error}`,
          timestamp: Date.now(),
        },
      ]);
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
      const result = (await invoke("load_chat_import")) as {
        success: boolean;
        data?: string;
        error?: string;
        messageCount?: number;
      };

      if (result.success && result.data) {
        const importData: ChatExport = JSON.parse(result.data);

        // Validate import format
        if (
          !importData.conversation ||
          !Array.isArray(importData.conversation)
        ) {
          throw new Error("Invalid chat export format");
        }

        // Confirm import with user
        const confirmImport = window.confirm(
          `Import ${
            result.messageCount || importData.conversation.length
          } messages? This will replace your current conversation.`
        );

        if (confirmImport) {
          // Add timestamps to imported messages if missing
          const importedMessages = importData.conversation.map((msg) => ({
            ...msg,
            timestamp: msg.timestamp || Date.now(),
          }));

          setConversation(importedMessages);

          setConversation((prev) => [
            ...prev,
            {
              role: "system",
              content: `✅ Chat imported successfully. Loaded ${importedMessages.length} messages.`,
              timestamp: Date.now(),
            },
          ]);
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
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Failed to import chat: ${error}`,
          timestamp: Date.now(),
        },
      ]);
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
          `**Priority:** ${feedbackData.priority}\n\n**Description:**\n${
            feedbackData.description
          }\n\n**Contact:** ${feedbackData.email || "Not provided"}`
        );
        const githubUrl = `https://github.com/lacymorrow/juno/issues/new?title=${title}&body=${body}`;

        // Open GitHub issues page
        await invoke("open_url", { url: githubUrl });
      } else {
        // For general feedback, create mailto link
        const subject = encodeURIComponent(
          `Juno AI Feedback: ${feedbackData.title}`
        );
        const body = encodeURIComponent(
          `Priority: ${feedbackData.priority}\n\nDescription:\n${feedbackData.description}`
        );
        const mailtoUrl = `mailto:feedback@juno-ai.com?subject=${subject}&body=${body}`;

        await invoke("open_url", { url: mailtoUrl });
      }

      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: "✅ Feedback form opened. Thank you for your input!",
          timestamp: Date.now(),
        },
      ]);

      // Reset form and close modal
      setFeedbackData({
        type: "general",
        title: "",
        description: "",
        email: "",
        priority: "medium",
      });
      setActiveModal(null);
    } catch (error) {
      console.error("❌ Failed to submit feedback:", error);
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Failed to open feedback form: ${error}`,
          timestamp: Date.now(),
        },
      ]);
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

  // Listen for agent-active events (hold mode)
  useEffect(() => {
    const unlisten = listen("agent-active", async (event) => {
      const isActive = event.payload as boolean;
      console.log("Received agent-active event:", isActive);

      if (isActive) {
        // Agent mode is activated - the transcription is already being handled
        // by the agent monitor in hold mode, so we don't need to call toggleDictation()
        console.log("Agent mode activated - transcription handled by backend");
      } else {
        // Agent mode ended
        console.log("Agent mode deactivated");
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

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

  // Listen for TTS stop requests from escape key
  useEffect(() => {
    const unlisten = listen("tts-stop-requested", async () => {
      console.log(
        "TTS stop requested event received - stopping TTS immediately"
      );
      try {
        // Immediately stop any currently playing audio
        if (currentAudio) {
          console.log("Stopping current audio element immediately");
          currentAudio.pause();
          currentAudio.currentTime = 0;
          if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
            URL.revokeObjectURL(currentAudio.src);
          }
          currentAudio.src = ""; // Clear the source
          setCurrentAudio(null);
          setCurrentAudioElement(null);
        }

        // Also call the TTS service stop function
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
  }, []); // Remove currentAudio dependency to avoid re-registering

  // Direct frontend escape key listener as backup for immediate TTS stopping
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        console.log(
          "Frontend escape key detected - stopping TTS audio immediately"
        );
        // Immediately stop any currently playing audio
        if (currentAudio) {
          console.log("Frontend: Stopping current audio element");
          currentAudio.pause();
          currentAudio.currentTime = 0;
          if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
            URL.revokeObjectURL(currentAudio.src);
          }
          currentAudio.src = "";
          setCurrentAudio(null);
          setCurrentAudioElement(null);
        }

        // Also call the TTS service stop function
        stopTTS((msg, level) =>
          console.log(`[Frontend TTS Stop-${level || "info"}] ${msg}`)
        ).catch((error) => {
          console.error("Frontend: Error stopping TTS:", error);
        });

        // Call our centralized stop function for backend operations
        invoke("stop_all_operations").catch((error) => {
          console.error(
            "Frontend: Error stopping all operations via escape key:",
            error
          );
        });
      }
    };

    // Add the event listener
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [currentAudio]); // Include currentAudio so we have the latest reference

  // Listen for agent events (thinking, tool calls, etc.)
  useEffect(() => {
    const unlistenPromise = listen<AgentEventTauri>("agent-event", (event) => {
      const { type, payload } = event.payload;
      const currentTime = Date.now();

      // NOTE: Toast notifications are now handled by the enhanced listener below
      // This listener only manages conversation state

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

  // Helper functions for tool categorization
  const isScreenshotTool = (toolName: string): boolean => {
    return (
      toolName.includes("screenshot") ||
      toolName.includes("capture") ||
      toolName === "screenshot" ||
      toolName === "capture_screenshot" ||
      toolName === "capture_element_screenshot" ||
      toolName === "browser_screenshot"
    );
  };

  const isFileOperationTool = (toolName: string): boolean => {
    return (
      toolName.includes("file") ||
      toolName.includes("read") ||
      toolName.includes("write") ||
      toolName.includes("list") ||
      toolName === "write_file" ||
      toolName === "read_file" ||
      toolName === "list_directory"
    );
  };

  const isBrowserTool = (toolName: string): boolean => {
    return (
      toolName.includes("browser") ||
      toolName.includes("navigate") ||
      toolName.includes("click") ||
      toolName.includes("web")
    );
  };

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const isSystemTool = (toolName: string): boolean => {
    return (
      toolName.includes("mouse") ||
      toolName.includes("keyboard") ||
      toolName.includes("key") ||
      toolName.includes("click") ||
      toolName.includes("type") ||
      toolName === "execute_shell_command"
    );
  };

  // @ts-ignore - Function may be used in future implementations
  const isImportantTool = (toolName: string): boolean => {
    return (
      isScreenshotTool(toolName) ||
      isFileOperationTool(toolName) ||
      isBrowserTool(toolName) ||
      isSystemTool(toolName)
    );
  };

  // @ts-ignore - Function may be used in future implementations
  const getFriendlyToolName = (toolName: string): string => {
    // Convert snake_case tool names to user-friendly names
    const friendlyNames: { [key: string]: string } = {
      capture_screenshot: "Taking screenshot",
      capture_element_screenshot: "Taking element screenshot",
      browser_screenshot: "Taking browser screenshot",
      write_file: "Writing file",
      read_file: "Reading file",
      list_directory: "Listing directory",
      browser_navigate: "Navigating to webpage",
      browser_click: "Clicking element",
      browser_type: "Typing in browser",
      execute_shell_command: "Running shell command",
      mouse_click: "Clicking mouse",
      mouse_move: "Moving mouse",
      key_press: "Pressing key",
      type_text: "Typing text",
    };

    return (
      friendlyNames[toolName] ||
      toolName.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase())
    );
  };

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

        // Smart auto-scroll during streaming (throttled)
        throttledAutoScroll();
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
  }, [throttledAutoScroll]); // Include throttledAutoScroll in dependency array

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitQuery(query);
  };

  // Handle stop button click - stop all operations
  const handleStop = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      console.log("Stop button clicked - stopping all operations");
      await invoke("stop_all_operations");
      console.log("Stop operations completed successfully");
    } catch (error) {
      console.error("Failed to stop operations:", error);
    }
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

  // Smart scroll conversation to bottom - only when user hasn't scrolled up
  useEffect(() => {
    autoScrollToBottom();
  }, [conversation, autoScrollToBottom]);

  // Force scroll to bottom when user sends a message or chat is cleared
  useEffect(() => {
    const lastMessage = conversation[conversation.length - 1];
    if (lastMessage?.role === "user" || conversation.length === 0) {
      // Always scroll to bottom when user sends a message or chat is empty
      autoScrollToBottom(true); // force scroll
    }
  }, [conversation, autoScrollToBottom]);

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
  const handleExamplePromptSelect = useCallback(
    (prompt: string) => {
      setQuery(prompt);
      // Auto-submit the selected prompt
      setTimeout(() => {
        const syntheticEvent = {
          preventDefault: () => {},
        } as React.FormEvent<HTMLFormElement>;
        handleSubmit(syntheticEvent);
      }, 100); // Small delay to ensure state is updated
    },
    [handleSubmit]
  );

  // Helper functions for ModalSystem
  const handleFeedbackDataChange = (data: Partial<FeedbackData>) => {
    setFeedbackData((prev) => ({ ...prev, ...data }));
  };

  const handleUpdateConversation = (newMessages: ChatMessage[]) => {
    setConversation(newMessages);
  };

  const handleAddSystemMessage = (content: string) => {
    setConversation((prev) => [
      ...prev,
      {
        role: "system",
        content,
        timestamp: Date.now(),
      },
    ]);
  };

  // Copy and Save handlers for agent responses with enhanced feedback
  const handleCopyResponse = useCallback(
    async (content: string, messageIndex: number) => {
      const messageId = `copy-${messageIndex}`;
      setCopyingMessageId(messageId);

      try {
        await navigator.clipboard.writeText(content);
        console.log("✅ Copied to clipboard successfully");
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: "✅ Response copied to clipboard",
            timestamp: Date.now(),
          },
        ]);
      } catch (error) {
        console.error("❌ Failed to copy to clipboard:", error);
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `❌ Failed to copy to clipboard: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        // Clear loading state after a brief delay for visual feedback
        setTimeout(() => setCopyingMessageId(null), 1000);
      }
    },
    []
  );

  const handleSaveResponse = useCallback(
    async (
      content: string,
      format: "html" | "markdown",
      messageIndex: number
    ) => {
      const messageId = `save-${format}-${messageIndex}`;
      setSavingMessageId(messageId);

      try {
        console.log(`💾 Saving response as ${format.toUpperCase()}...`);
        const filePath = await invoke("save_agent_response", {
          content,
          format,
          suggested_filename: `agent_response_${Date.now()}`,
        });
        console.log(`✅ Response saved to: ${filePath}`);
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `✅ Response saved as ${format.toUpperCase()} to: ${filePath}`,
            timestamp: Date.now(),
          },
        ]);
      } catch (error) {
        console.error(`❌ Failed to save response as ${format}:`, error);
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: `❌ Failed to save response as ${format.toUpperCase()}: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        // Clear loading state after a brief delay for visual feedback
        setTimeout(() => setSavingMessageId(null), 1000);
      }
    },
    []
  );

  // Enhanced agent event listener for dynamic tool notifications
  useEffect(() => {
    const unlistenAgentEvent = listen<{
      type: string;
      payload: {
        tool_name?: string;
        tool_category?: string;
        tool_description?: string;
        notification_level?: string;
        estimated_duration?: string;
        execution_time_ms?: number;
        success?: boolean;
        content?: string;
        screenshot_base64?: string;
        [key: string]: any;
      };
    }>("agent-event", (event) => {
      const { type: eventType, payload } = event.payload;

      // Handle different event types with dynamic metadata
      switch (eventType) {
        case "tool_call_request": {
          const notificationLevel = payload.notification_level || "standard";

          // Only show notifications for non-silent tools
          if (notificationLevel !== "silent") {
            const message =
              payload.content || `🔧 Executing ${payload.tool_name}...`;
            const duration = getNotificationDuration(
              notificationLevel,
              payload.estimated_duration
            );

            toast.info(message, {
              duration,
              className: getNotificationClassName(
                payload.tool_category,
                "request"
              ),
            });
          }
          break;
        }

        case "tool_call_result": {
          const notificationLevel = payload.notification_level || "standard";
          const success = payload.success ?? true;

          // Only show notifications for non-silent tools
          if (notificationLevel !== "silent") {
            const message =
              payload.content ||
              (success ? `✅ Tool completed` : `❌ Tool failed`);

            const duration = getNotificationDuration(notificationLevel);
            const toastType = success ? "success" : "error";

            toast[toastType](message, {
              duration,
              className: getNotificationClassName(
                payload.tool_category,
                "result",
                success
              ),
            });

            // Special handling for screenshot results
            if (payload.screenshot_base64 && success) {
              toast.success("📸 Screenshot captured", {
                duration: 3000,
                className: "screenshot-notification",
              });
            }
          }
          break;
        }

        case "thinking": {
          if (payload.content) {
            toast.info(`💭 ${payload.content}`, {
              duration: 2000,
              className: "thinking-notification",
            });
          }
          break;
        }

        case "screenshot": {
          toast.success("📸 Screenshot captured", {
            duration: 3000,
            className: "screenshot-notification",
          });
          break;
        }

        case "generic_content": {
          if (payload.content) {
            toast.info(payload.content, {
              duration: 4000,
              className: "generic-content-notification",
            });
          }
          break;
        }

        default:
          // Handle unknown event types gracefully
          console.log("Unknown agent event type:", eventType, payload);
          break;
      }
    });

    return () => {
      unlistenAgentEvent.then((unlisten) => unlisten());
    };
  }, []);

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

  // Improved auto-scroll function
  const autoScrollToBottom = useCallback(
    (forceScroll = false) => {
      if (!conversationEndRef.current) return;

      // Don't auto-scroll if user has scrolled up, unless forced
      if (userHasScrolledUp && !forceScroll) return;

      // Smooth scroll to bottom
      conversationEndRef.current.scrollIntoView({ behavior: "smooth" });
      setLastScrollTime(Date.now());
    },
    [userHasScrolledUp]
  );

  // Throttled scroll function for streaming (limits frequency)
  const throttledAutoScroll = useCallback(
    debounce(() => autoScrollToBottom(), 200), // Only scroll every 200ms during streaming
    [autoScrollToBottom]
  );

  // Add scroll detection
  const handleScroll = useCallback(() => {
    const scrollElement = scrollAreaRef.current?.querySelector(
      "[data-radix-scroll-area-viewport]"
    );
    if (!scrollElement) return;

    const { scrollTop, scrollHeight, clientHeight } = scrollElement;
    const scrollBottom = scrollHeight - scrollTop - clientHeight;

    // Consider user at bottom if within 100px of bottom
    const isNearBottom = scrollBottom < 100;

    // Update user scroll state
    if (!isNearBottom && Date.now() - lastScrollTime > 500) {
      // User scrolled up and it's been more than 500ms since last auto-scroll
      setUserHasScrolledUp(true);
    } else if (isNearBottom) {
      // User is at bottom, resume auto-scrolling
      setUserHasScrolledUp(false);
    }
  }, [lastScrollTime]);

  // Add scroll event listener to detect user scroll behavior
  useEffect(() => {
    const scrollElement = scrollAreaRef.current?.querySelector(
      "[data-radix-scroll-area-viewport]"
    );
    if (!scrollElement) return;

    scrollElement.addEventListener("scroll", handleScroll);
    return () => scrollElement.removeEventListener("scroll", handleScroll);
  }, [handleScroll]);

  return (
    <VoiceProvider>
      <main className="h-screen flex flex-col">
        {/* Click Visualizer - overlays the entire app to show click indicators (from tools2) */}
        <ClickVisualizer />
        <KeyPressOverlay />
        <CommandOverlay />

        {/* Tool Approval Modal - for requesting user approval before tool execution */}
        <ToolApprovalModal />

        <div className="w-screen h-screen bg-background text-foreground">
          <div className="container mx-auto p-2 h-full flex flex-col">
            {/* Header */}
            <AppHeader
              serverStatus={serverStatus}
              isProcessing={isProcessing}
              currentView={currentView}
              isDevPanelOpen={isDevPanelOpen}
              onViewChange={setCurrentView}
              onToggleDevPanel={() => setIsDevPanelOpen(!isDevPanelOpen)}
            />

            {/* Main Content Area - Conditional based on current view */}
            {currentView === "devtools" ? (
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
                    <ScrollArea
                      className="flex-1 min-h-0 mb-2 -mr-4 pr-4"
                      ref={scrollAreaRef}
                    >
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
                              key={`msg-container-${index}-${
                                msg.timestamp || Date.now()
                              }`}
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

                              <ChatMessageComponent
                                msg={msg}
                                index={index}
                                copyingMessageId={copyingMessageId}
                                savingMessageId={savingMessageId}
                                onCopyResponse={handleCopyResponse}
                                onSaveResponse={handleSaveResponse}
                              />
                            </div>
                          );
                        })
                      )}
                      <div ref={conversationEndRef} />
                    </ScrollArea>

                    {/* Input Form */}
                    <AIInput
                      onSubmit={isProcessing ? handleStop : handleSubmit}
                    >
                      <AIInputTextarea
                        name="message"
                        placeholder={
                          isProcessing
                            ? "Processing..."
                            : "What would you like to know?"
                        }
                        value={query}
                        onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
                          setQuery(e.target.value)
                        }
                        disabled={isProcessing || serverStatus !== "connected"}
                        minHeight={48}
                        maxHeight={164}
                      />
                      <AIInputToolbar>
                        <AIInputTools>
                          <AIInputButton
                            onClick={startNewChat}
                            disabled={isProcessing}
                            title="Start new agent chat"
                          >
                            <Plus size={18} />
                            New Chat
                          </AIInputButton>
                          <AIInputButton
                            onClick={clearConversation}
                            disabled={isProcessing}
                            title="Clear conversation history"
                          >
                            <Trash2 size={18} />
                            Clear
                          </AIInputButton>
                        </AIInputTools>
                        <AIInputSubmit
                          disabled={
                            !isProcessing &&
                            (serverStatus !== "connected" || !query.trim())
                          }
                          variant={isProcessing ? "destructive" : "default"}
                          title={
                            isProcessing
                              ? "Stop all operations"
                              : "Submit query"
                          }
                        >
                          {isProcessing ? (
                            <Square size={18} />
                          ) : (
                            <Send size={18} />
                          )}
                        </AIInputSubmit>
                      </AIInputToolbar>
                    </AIInput>
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
        <ModalSystem
          activeModal={activeModal}
          onClose={() => setActiveModal(null)}
          feedbackData={feedbackData}
          onFeedbackDataChange={handleFeedbackDataChange}
          updateInfo={updateInfo}
          conversation={conversation}
          isExporting={isExporting}
          isImporting={isImporting}
          keyboardShortcuts={keyboardShortcuts}
          onUpdateConversation={handleUpdateConversation}
          onAddSystemMessage={handleAddSystemMessage}
        />

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
    </VoiceProvider>
  );
}

export default App;
