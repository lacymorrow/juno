import ClickVisualizer from "@/components/ClickVisualizer"; // Import the ClickVisualizer
import DevToolsPanel from "@/components/DevToolsPanel"; // Import the new panel
import { PermissionsFlow } from "@/components/PermissionsFlow"; // Import the PermissionsFlow component
import Settings from "@/components/Settings"; // Import the Settings component
import { Button } from "@/components/ui/button"; // Shadcn Button
import { Input } from "@/components/ui/input"; // Shadcn Input
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"; // Import Resizable components
import { ScrollArea } from "@/components/ui/scroll-area"; // Import Shadcn ScrollArea
import { useSound, useVoiceSounds } from "@/hooks/useSound"; // Import sound hooks
import { cn } from "@/lib/utils"; // Shadcn utility
import { invoke } from "@tauri-apps/api/core"; // Use Tauri's invoke
import { listen } from "@tauri-apps/api/event"; // Import listen
import {
  ArrowLeft,
  DogIcon,
  PanelLeftClose,
  PanelLeftOpen,
  Send,
  Server,
  Trash2,
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
  timestamp?: number; // Add timestamp field for message grouping
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

  // Debounced handler function
  const handleBackendResponseDebounced = useCallback(
    debounce((payload: BackendResponsePayload) => {
      console.log("Debounced handler executing for:", payload.query);
      const { query, response } = payload;

      // Add user query message with timestamp
      const currentTime = Date.now();
      const userMessage: ChatMessage = {
        role: "user",
        content: query,
        timestamp: currentTime,
      };
      // Add assistant response message with screenshot if available
      const assistantMessage: ChatMessage = {
        role: "assistant",
        content: response.text,
        screenshot_base64: response.screenshot_base64,
        timestamp: currentTime + 100, // Slight offset to ensure ordering
      };

      setConversation((prev) => [...prev, userMessage, assistantMessage]);

      // Play audio if available
      if (response.audio_base64) {
        playAudioFromBase64(response.audio_base64); // This function already handles stopping previous audio
      }

      // Note: Sound feedback is now handled by the Rust backend based on agent_state
      // No need for frontend sound calls here to avoid duplicates

      // Potentially reset processing state if managed globally here
      setIsProcessing(false); // Assuming Bar.tsx also sets this true
    }, 100), // Debounce for 100ms
    [] // Remove sound from dependencies since we're not using it here anymore
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

  // Add tool usage event listener to update the conversation with screenshots
  useEffect(() => {
    const unlisten = listen<any>("agent-event", (event) => {
      console.log("Received agent-event:", event.payload);
      const { type, payload } = event.payload;

      let newMessage: ChatMessage | null = null;

      const eventTimestamp = Date.now();

      if (type === "thinking" && payload.content) {
        newMessage = {
          role: "thinking",
          content: payload.content,
          timestamp: eventTimestamp,
        };
      } else if (
        type === "tool_call_request" &&
        payload.tool_name &&
        payload.tool_args
      ) {
        newMessage = {
          role: "tool_call_request",
          content: payload.content || `Calling tool: ${payload.tool_name}`,
          tool_name: payload.tool_name,
          tool_args: payload.tool_args,
          timestamp: eventTimestamp,
        };
      } else if (
        type === "tool_call_result" &&
        payload.tool_name &&
        payload.tool_output
      ) {
        newMessage = {
          role: "tool_call_result",
          content: payload.content || `Result from ${payload.tool_name}`,
          tool_name: payload.tool_name,
          tool_output: payload.tool_output,
          screenshot_base64: payload.screenshot_base64, // Include screenshot if part of tool result
          timestamp: eventTimestamp,
        };
      } else if (type === "screenshot" && payload.screenshot_base64) {
        // This can also be a specific type of tool_call_result if a screenshot tool was called
        // Or a general system message if the AI decides to capture one proactively.
        newMessage = {
          role: "system", // Or could be 'tool_call_result' if tied to a specific tool action
          content: payload.content || "The AI captured a screenshot.",
          screenshot_base64: payload.screenshot_base64,
          timestamp: eventTimestamp,
        };
      }

      if (newMessage) {
        setConversation((prev) => [...prev, newMessage!]);
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

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
          // Play error sound for failed transcription
          voiceSounds.playVoiceError().catch(console.error);
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
          // Only play sound for successful agent mode transcription (not spacebar dictation)
          // Spacebar dictation handles its own immediate typing feedback
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

        // Play appropriate sound based on dictation state
        if (isNowDictating) {
          voiceSounds.playVoiceStart().catch(console.error);
        } else {
          voiceSounds.playVoiceEnd().catch(console.error);
        }
      } catch (error) {
        console.error("Failed to toggle dictation:", error);
        // Play error sound for failed toggle
        sound.playError().catch(console.error);
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
          handleBackendResponseDebounced(event.payload);
        }
      );
    };

    setupListener();

    // Cleanup listener on component unmount
    return () => {
      unlisten?.();
    };
  }, [handleBackendResponseDebounced]); // Add debounced handler to dependency array

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
      currentAudio.src = ""; // Release object URL implicitly via new assignment below
    }

    try {
      const audioBlob = base64ToBlob(base64Audio);
      const audioUrl = URL.createObjectURL(audioBlob);
      const newAudio = new Audio(audioUrl);
      setCurrentAudio(newAudio); // Store the new audio element

      newAudio.play();

      newAudio.onended = () => {
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
      newAudio.onerror = (e) => {
        console.error("Audio playback error:", e);
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
    } catch (error) {
      console.error("Error processing or playing audio:", error);
      setCurrentAudio(null);
    }
  };

  // Clear conversation history
  const clearConversation = async () => {
    try {
      await invoke("clear_conversation_history");
      setConversation([
        {
          role: "system",
          content:
            "Conversation history cleared. You can start a new conversation.",
          timestamp: Date.now(),
        },
      ]);
      console.log("Conversation history cleared successfully");
    } catch (error) {
      console.error("Failed to clear conversation history:", error);
      setConversation((prev) => [
        ...prev,
        {
          role: "system",
          content: `Error clearing conversation: ${error}`,
          timestamp: Date.now(),
        },
      ]);
    }
  };

  // Cleanup effect for audio
  useEffect(() => {
    return () => {
      if (currentAudio) {
        currentAudio.pause();
        if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
          URL.revokeObjectURL(currentAudio.src);
        }
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
              <ResizablePanel defaultSize={75} minSize={30}>
                <div className="flex flex-col h-full p-4">
                  {/* Conversation Area */}
                  <ScrollArea className="flex-1 min-h-0 mb-4 -mr-4 pr-4">
                    {conversation.map((msg, index) => {
                      const previousMsg =
                        index > 0 ? conversation[index - 1] : null;
                      const showTimestamp = shouldShowTimestamp(
                        msg,
                        previousMsg
                      );

                      return (
                        <div key={index}>
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
                                  : (msg.role === "system" ||
                                      msg.role === "thinking" ||
                                      msg.role === "tool_call_request" ||
                                      msg.role === "tool_call_result") &&
                                    msg.screenshot_base64
                                  ? "bg-muted/80 border border-primary/20 p-2"
                                  : msg.role === "thinking"
                                  ? "bg-blue-100 text-blue-800 text-xs italic opacity-90"
                                  : msg.role === "tool_call_request"
                                  ? "bg-purple-100 text-purple-800 text-xs"
                                  : msg.role === "tool_call_result"
                                  ? "bg-green-100 text-green-800 text-xs"
                                  : "bg-secondary text-secondary-foreground text-xs italic opacity-80" // Default system
                              )}
                            >
                              {msg.role === "thinking" && (
                                <div className="font-semibold mb-1">
                                  Thinking...
                                </div>
                              )}
                              {msg.role === "tool_call_request" &&
                                msg.tool_name && (
                                  <div className="font-semibold mb-1">
                                    Tool Call:{" "}
                                    <code className="font-mono bg-purple-200 px-1 rounded">
                                      {msg.tool_name}
                                    </code>
                                  </div>
                                )}
                              {msg.role === "tool_call_result" &&
                                msg.tool_name && (
                                  <div className="font-semibold mb-1">
                                    Tool Result:{" "}
                                    <code className="font-mono bg-green-200 px-1 rounded">
                                      {msg.tool_name}
                                    </code>
                                  </div>
                                )}
                              {msg.content}
                              {msg.role === "tool_call_request" &&
                                msg.tool_args && (
                                  <pre className="mt-1 p-1.5 bg-purple-50 text-xs rounded overflow-x-auto">
                                    Args:{" "}
                                    {JSON.stringify(msg.tool_args, null, 2)}
                                  </pre>
                                )}
                              {msg.role === "tool_call_result" &&
                                msg.tool_output && (
                                  <pre className="mt-1 p-1.5 bg-green-50 text-xs rounded overflow-x-auto">
                                    Output:{" "}
                                    {typeof msg.tool_output === "string"
                                      ? msg.tool_output
                                      : JSON.stringify(
                                          msg.tool_output,
                                          null,
                                          2
                                        )}
                                  </pre>
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
                            </span>
                          </div>
                        </div>
                      );
                    })}
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

              {/* Resizable Handle */}
              <ResizableHandle withHandle />

              {/* Dev Tools & Logs Panel (Collapsible) */}
              <ResizablePanel
                collapsible
                collapsedSize={0} // Completely collapses
                minSize={50} // Minimum size when expanded - Updated min size from main
                defaultSize={100} // Default size when expanded - Updated default size from main
                className={cn(
                  isDevPanelOpen ? "block" : "hidden",
                  "overflow-hidden" // Ensure panel itself doesn't scroll
                )}
              >
                {/* Apply ScrollArea directly inside the panel */}
                <ScrollArea className="h-full w-full p-3">
                  {" "}
                  {/* Full size and padding */}
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
            </ResizablePanelGroup>
          )}
        </div>
      </div>
    </main>
  );
}

export default App;
