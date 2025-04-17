import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core"; // Use Tauri's invoke
import {
  Send,
  Server,
  BotMessageSquare,
  Bug,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react"; // Icons
import { Button } from "@/components/ui/button"; // Shadcn Button
import { Input } from "@/components/ui/input"; // Shadcn Input
import { ScrollArea } from "@/components/ui/scroll-area"; // Import Shadcn ScrollArea
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"; // Import Shadcn Card
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"; // Import Resizable components
import { cn } from "@/lib/utils"; // Shadcn utility
import DevToolsPanel from "@/components/DevToolsPanel"; // Import the new panel

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

// Type for logs
type LogEntry = {
  level: string;
  message: string;
  timestamp: number;
};

function App() {
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus, setServerStatus] = useState<
    "checking" | "connected" | "error"
  >("checking");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false); // State for collapsible panel
  const conversationEndRef = useRef<HTMLDivElement>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(
    null
  );

  // Client-side log function
  const addLog = (message: string, level: string = "info") => {
    const newLog: LogEntry = { level, message, timestamp: Date.now() };
    setLogs((prev) => [...prev, newLog]);
    console.log(`[${level}] ${message}`);
  };

  // Check server status on mount
  useEffect(() => {
    const checkServer = async () => {
      addLog("checking backend status...");
      try {
        const isConnected: boolean = await invoke("check_server_status");
        if (isConnected) {
          setServerStatus("connected");
          addLog("connected to backend", "success");
          setConversation([
            {
              role: "system",
              content: "Connected. Enter your query below.",
            },
          ]);
        } else {
          setServerStatus("error");
          addLog("backend check failed (returned false)", "error");
          setConversation([
            {
              role: "system",
              content: "Failed to connect to backend. Please check logs.",
            },
          ]);
        }
      } catch (error) {
        setServerStatus("error");
        addLog(`failed to invoke 'check_server_status': ${error}`, "error");
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

  // Submit query using Tauri invoke
  const submitQuery = async (text: string) => {
    if (!text.trim() || isProcessing || serverStatus !== "connected") {
      addLog(
        `submission skipped: ${
          !text.trim()
            ? "empty query"
            : isProcessing
            ? "already processing"
            : "server disconnected"
        }`,
        "warn"
      );
      return;
    }

    const userMessage: ChatMessage = { role: "user", content: text };
    setConversation((prev) => [...prev, userMessage]);
    setQuery(""); // Clear input after sending
    addLog(`sending query: ${text}`);
    setIsProcessing(true);

    try {
      // Expecting an object with text and optional audio_base64
      const result: SubmitQueryResult = await invoke("submit_query", {
        query: text,
      });
      addLog("received response from backend", "info");

      const assistantMessage: ChatMessage = {
        role: "assistant",
        content: result.text, // Use the text part of the response
      };
      setConversation((prev) => [...prev, assistantMessage]);

      // Play audio if available
      if (result.audio_base64) {
        addLog("Audio data received, attempting playback.", "info");
        playAudioFromBase64(result.audio_base64);
      } else {
        addLog("No audio data received with the response.", "info");
      }
    } catch (error) {
      addLog(`query invocation error: ${error}`, "error");
      const errorMessage: ChatMessage = {
        role: "system",
        content: `Error processing query: ${error}`,
      };
      setConversation((prev) => [...prev, errorMessage]);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitQuery(query);
  };

  // Fetch logs periodically
  useEffect(() => {
    if (serverStatus !== "connected") return;

    const fetchBackendLogs = async () => {
      try {
        const backendLogs: string[] = await invoke("get_logs");

        if (backendLogs && backendLogs.length > 0) {
          const newLogEntries: LogEntry[] = backendLogs.map((logMsg) => {
            const match = logMsg.match(/^\[(.*?)\]\s*(.*)$/);
            if (match) {
              return {
                level: match[1],
                message: match[2],
                timestamp: Date.now(), // Assign timestamp on arrival
              };
            } else {
              return {
                level: "backend",
                message: logMsg,
                timestamp: Date.now(),
              };
            }
          });

          setLogs((prevLogs) => {
            const existingTimestamps = new Set(
              prevLogs.map((log) => log.timestamp)
            );
            const uniqueNewLogs = newLogEntries.filter(
              (newLog) => !existingTimestamps.has(newLog.timestamp)
            );
            return [...prevLogs, ...uniqueNewLogs].sort(
              (a, b) => a.timestamp - b.timestamp
            );
          });
        }
      } catch (error) {
        // Avoid logging the fetch error itself to prevent loops if get_logs fails
        console.error("Error fetching backend logs:", error);
      }
    };

    fetchBackendLogs();
    const interval = setInterval(fetchBackendLogs, 3000); // Poll every 3 seconds

    return () => clearInterval(interval);
  }, [serverStatus]);

  // Helper to get color class based on log level
  const getLogColorClass = (level: string): string => {
    switch (level.toLowerCase()) {
      case "error":
        return "text-red-500 dark:text-red-400";
      case "warn":
        return "text-yellow-500 dark:text-yellow-400";
      case "success":
        return "text-green-500 dark:text-green-400";
      case "backend":
      case "debug":
        return "text-purple-500 dark:text-purple-400";
      case "info":
      default:
        return "text-blue-500 dark:text-blue-400";
    }
  };

  // Helper to format timestamp
  const formatTimestamp = (timestamp: number): string => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
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
      addLog("Stopped previous audio playback.", "debug");
    }

    try {
      const audioBlob = base64ToBlob(base64Audio);
      const audioUrl = URL.createObjectURL(audioBlob);
      const newAudio = new Audio(audioUrl);
      setCurrentAudio(newAudio); // Store the new audio element

      newAudio.play();
      addLog("Starting audio playback.", "info");

      newAudio.onended = () => {
        addLog("Audio playback finished.", "info");
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
      newAudio.onerror = (e) => {
        console.error("Audio playback error:", e);
        addLog(`Audio playback error: ${e}`, "error");
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
    } catch (error) {
      console.error("Error processing or playing audio:", error);
      addLog(`Failed to process or play audio: ${error}`, "error");
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
        addLog("Cleaned up audio on component unmount.", "debug");
      }
    };
  }, [currentAudio]);

  // Scroll conversation to bottom
  useEffect(() => {
    conversationEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [conversation]);

  return (
    <div className="container mx-auto p-4 h-screen flex flex-col bg-background text-foreground">
      {/* Header */}
      <header className="flex justify-between items-center mb-4 flex-shrink-0 border-b pb-2">
        <h1 className="text-xl font-semibold flex items-center gap-2">
          <BotMessageSquare size={24} /> DotDot AI Assistant
        </h1>
        <div className="flex items-center gap-3">
          {/* Status Indicator */}
          <div className="flex items-center gap-1 text-sm">
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
              {" "}
              {/* Adjust margin/padding for scrollbar */}
              {conversation.map((msg, index) => (
                <div
                  key={index}
                  className={`mb-3 flex ${
                    msg.role === "user" ? "justify-end" : "justify-start"
                  }`}
                >
                  <span
                    className={cn(
                      "inline-block max-w-[80%] px-3 py-1.5 rounded-lg", // Added max-width
                      msg.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : msg.role === "assistant"
                        ? "bg-muted"
                        : "bg-secondary text-secondary-foreground text-xs italic"
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
                  isProcessing || serverStatus !== "connected" || !query.trim()
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
          minSize={15} // Minimum size when expanded
          defaultSize={25} // Default size when expanded
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
              {" "}
              {/* Spacing and border */}
              <DevToolsPanel />
            </div>
            {/* Logs Area */}
            <div className="flex-grow">
              {" "}
              {/* Logs take remaining space */}
              {logs.map((log, index) => (
                <div
                  key={index}
                  className={cn(
                    "text-xs mb-1 font-mono whitespace-pre-wrap",
                    getLogColorClass(log.level)
                  )}
                >
                  <span className="text-muted-foreground mr-1">
                    [{formatTimestamp(log.timestamp)}]
                  </span>
                  <span className="font-semibold mr-1">
                    [{log.level.toUpperCase()}]
                  </span>
                  {log.message}
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          </ScrollArea>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

export default App;
