import DevToolsPanel from "@/components/DevToolsPanel"; // Import the new panel
import { Button } from "@/components/ui/button"; // Shadcn Button
import { Input } from "@/components/ui/input"; // Shadcn Input
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"; // Import Resizable components
import { ScrollArea } from "@/components/ui/scroll-area"; // Import Shadcn ScrollArea
import { cn } from "@/lib/utils"; // Shadcn utility
import { invoke } from "@tauri-apps/api/core"; // Use Tauri's invoke
import { listen } from "@tauri-apps/api/event"; // Import listen
import {
  BotMessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Send,
  Server,
} from "lucide-react"; // Icons
import { useCallback, useEffect, useRef, useState } from "react";

// Type for conversation messages
type ChatMessage = {
  role: "user" | "assistant" | "system";
  content: string;
};

// Type for the result from submit_query
type SubmitQueryResult = {
  text: string;
  audio_base64?: string; // Optional base64 audio data
};

// Type for the backend response event payload
type BackendResponsePayload = {
  query: string;
  response: SubmitQueryResult;
};

// --- Payload Types for Backend Events ---
type AssistantTextDeltaPayload = {
  delta: string;
};

// Type for the result part of the final response
type FinalAssistantResponsePayload = {
  query: string;
  response: SubmitQueryResult;
};

function App() {
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus, setServerStatus] = useState<
    "checking" | "connected" | "error"
  >("checking");
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false); // State for collapsible panel
  const conversationEndRef = useRef<HTMLDivElement>(null);
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(
    null
  );

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
    let unlistenTextDelta: (() => void) | undefined;
    let unlistenFinalResponse: (() => void) | undefined;

    const setupListener = async () => {
      // Listener for text chunks
      unlistenTextDelta = await listen<AssistantTextDeltaPayload>(
        "assistant-text-delta",
        (event) => {
          console.log("Received text delta:", event.payload.delta);
          setConversation((prev) => {
            const lastMessage = prev[prev.length - 1];
            // If last message is from assistant, append delta
            if (lastMessage?.role === "assistant") {
              const updatedMessage = {
                ...lastMessage,
                content: lastMessage.content + event.payload.delta,
              };
              return [...prev.slice(0, -1), updatedMessage];
            } else {
              // Otherwise, create a new assistant message
              const newAssistantMessage: ChatMessage = {
                role: "assistant",
                content: event.payload.delta,
              };
              return [...prev, newAssistantMessage];
            }
          });
        }
      );

      // Listener for the final response (includes full text and audio)
      unlistenFinalResponse = await listen<FinalAssistantResponsePayload>(
        "final-assistant-response",
        (event) => {
          console.log(
            "Received final response for query:",
            event.payload.query
          );
          const { response } = event.payload;
          // Update the last message with the final text (optional, if deltas didn't cover everything)
          // Or just rely on deltas having built the full message.
          setConversation((prev) => {
            const lastMessage = prev[prev.length - 1];
            if (lastMessage?.role === "assistant") {
              // Ensure the final text is set correctly, correcting any minor discrepancies
              const finalMessage = { ...lastMessage, content: response.text };
              return [...prev.slice(0, -1), finalMessage];
            } else {
              // If no assistant message was streamed (e.g., error or immediate empty response),
              // create one now.
              const finalAssistantMessage: ChatMessage = {
                role: "assistant",
                content: response.text,
              };
              return [...prev, finalAssistantMessage];
            }
          });

          // Play audio if available
          if (response.audio_base64) {
            playAudioFromBase64(response.audio_base64);
          }

          // Reset processing state
          setIsProcessing(false);
        }
      );
    };

    setupListener();

    // Cleanup listener on component unmount
    return () => {
      unlistenTextDelta?.();
      unlistenFinalResponse?.();
    };
  }, []); // Re-run only on mount/unmount

  // Submit query using Tauri invoke (primarily for the main input)
  const submitQuery = async (text: string) => {
    if (!text.trim() || isProcessing || serverStatus !== "connected") {
      return;
    }

    // Add user message immediately for better perceived responsiveness
    const userMessage: ChatMessage = { role: "user", content: text };
    setConversation((prev) => [...prev, userMessage]);

    setQuery(""); // Clear input immediately
    setIsProcessing(true); // Set processing state

    try {
      // Invoke the backend command. We assume it triggers the "backend-response" event.
      // The direct return value might be empty or just a confirmation now.
      await invoke("submit_query", { query: text });
      console.log("submit_query invoked for:", text);
      // Response handling is now done via the event listener.
    } catch (error) {
      console.error("Error invoking submit_query:", error);
      const errorMessage: ChatMessage = {
        role: "system",
        content: `Error invoking submit_query: ${error}`,
      };
      setConversation((prev) => [...prev, errorMessage]);
      setIsProcessing(false); // Reset processing on error
    }
  };

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
    <div className="w-screen h-screen bg-background text-foreground">
      <div className="container mx-auto p-4 h-full flex flex-col">
        {/* Header */}
        <header className="flex justify-between items-center mb-4 flex-shrink-0 border-b pb-2">
          <h1 className="text-xl font-semibold flex items-center gap-2">
            <BotMessageSquare size={24} /> Juno{" "}
            <span className="text-xs text-muted-foreground">Operator</span>
          </h1>
          <div className="flex items-center gap-4">
            {/* Status Indicator */}
            <div className="flex items-center gap-1.5 text-sm text-muted-foreground">
              <Server
                size={16}
                className={cn(
                  serverStatus === "connected"
                    ? "text-green-500"
                    : serverStatus === "error"
                    ? "text-red-500"
                    : "text-yellow-500 animate-pulse"
                )}
              />
              {serverStatus === "connected"
                ? "Connected"
                : serverStatus === "error"
                ? "Connection Error"
                : "Connecting..."}
            </div>
            {/* Toggle Dev Panel Button */}
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
          </div>
        </header>

        {/* Main Content Area (Resizable Chat + Dev Panel) */}
        <ResizablePanelGroup
          direction="horizontal"
          className="flex-grow rounded-lg border overflow-hidden"
        >
          {/* Chat Panel */}
          <ResizablePanel defaultSize={75} minSize={30}>
            <div className="flex flex-col h-full p-4">
              {/* Conversation Area */}
              <ScrollArea className="flex-grow mb-4 -mr-4 pr-4">
                {conversation.map((msg, index) => (
                  <div
                    key={index}
                    className={`mb-3 flex ${
                      msg.role === "user" ? "justify-end" : "justify-start"
                    }`}
                  >
                    <span
                      className={cn(
                        "inline-block max-w-[85%] px-3 py-1.5 rounded-lg shadow-sm",
                        msg.role === "user"
                          ? "bg-primary text-primary-foreground"
                          : msg.role === "assistant"
                          ? "bg-muted"
                          : "bg-secondary text-secondary-foreground text-xs italic opacity-80"
                      )}
                    >
                      {msg.content}
                    </span>
                  </div>
                ))}
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
            minSize={50} // Minimum size when expanded
            defaultSize={100} // Default size when expanded
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
      </div>
    </div>
  );
}

export default App;
