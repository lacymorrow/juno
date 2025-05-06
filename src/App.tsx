import ClickVisualizer from "@/components/ClickVisualizer"; // Import the ClickVisualizer
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
  screenshot_base64?: string; // Optional base64 screenshot data
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

  // Debounced handler function
  const handleBackendResponseDebounced = useCallback(
    debounce((payload: BackendResponsePayload) => {
      console.log("Debounced handler executing for:", payload.query);
      const { query, response } = payload;

      // Add user query message
      const userMessage: ChatMessage = { role: "user", content: query };
      // Add assistant response message with screenshot if available
      const assistantMessage: ChatMessage = {
        role: "assistant",
        content: response.text,
        screenshot_base64: response.screenshot_base64,
      };

      setConversation((prev) => [...prev, userMessage, assistantMessage]);

      // Play audio if available
      if (response.audio_base64) {
        playAudioFromBase64(response.audio_base64); // This function already handles stopping previous audio
      }

      // Potentially reset processing state if managed globally here
      setIsProcessing(false); // Assuming Bar.tsx also sets this true
    }, 100), // Debounce for 100ms
    [] // Dependencies for useCallback
    // Note: If playAudioFromBase64 relies on state/props, add them here or wrap playAudioFromBase64 in useCallback too.
    // For now, assuming playAudioFromBase64 is stable or relies only on its arguments and `currentAudio` state/setter.
  );

  // Add tool usage event listener to update the conversation with screenshots
  useEffect(() => {
    const unlisten = listen<any>("tool-usage", (event) => {
      // Only process screenshot tools
      if (
        (event.payload.tool === "capture_screenshot" ||
          event.payload.tool === "screenshot") &&
        event.payload.screenshot_base64 &&
        event.payload.success
      ) {
        console.log("Received screenshot from tool usage");

        // Add system message with the screenshot
        const screenshotMessage: ChatMessage = {
          role: "system",
          content:
            "The AI captured a screenshot of your screen to help complete your request.",
          screenshot_base64: event.payload.screenshot_base64,
        };

        setConversation((prev) => [...prev, screenshotMessage]);
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

  // Submit query using Tauri invoke (primarily for the main input)
  // Note: This function might need adjustment if the backend
  // `submit_query` command no longer returns the result directly.
  // For now, we assume it might still be used by the main input,
  // OR that the main input also triggers the event flow.
  // If `submit_query` backend now ONLY emits, this function needs adjustment.
  const submitQuery = async (text: string) => {
    if (!text.trim() || isProcessing || serverStatus !== "connected") {
      return;
    }

    // Optimistically add user message? Or wait for event?
    // Let's wait for the event to handle both user and assistant messages consistently.
    // const userMessage: ChatMessage = { role: "user", content: text };
    // setConversation((prev) => [...prev, userMessage]);
    setQuery(""); // Clear input immediately
    setIsProcessing(true); // Set processing state

    try {
      // Invoke the backend command. We assume it triggers the "backend-response" event.
      // The direct return value might be empty or just a confirmation now.
      await invoke("submit_query", { query: text });
      console.log("submit_query invoked for:", text);
      // Response handling is now done via the event listener.
    } catch (error) {
      const errorMessage: ChatMessage = {
        role: "system",
        content: `Error invoking submit_query: ${error}`,
      };
      setConversation((prev) => [...prev, errorMessage]);
      setIsProcessing(false); // Reset processing on error
    }
    // No finally block to set isProcessing(false) here, as the event listener handles it on success.
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
    <main className="h-screen flex flex-col">
      {/* Click Visualizer - overlays the entire app to show click indicators (from tools2) */}
      <ClickVisualizer />

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
                            : msg.role === "system" && msg.screenshot_base64
                            ? "bg-muted/80 border border-primary/20 p-2"
                            : "bg-secondary text-secondary-foreground text-xs italic opacity-80"
                        )}
                      >
                        {msg.content}
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
        </div>
      </div>
    </main>
  );
}

export default App;
