import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { 
  AppView, 
  AgentEventTauri, 
  BackendResponsePayload, 
  StreamingTextEvent, 
  StreamStartEvent, 
  StreamEndEvent,
  ToolCallRequestPayload,
  ToolCallResultPayload,
  ScreenshotPayload
} from "@/types/chat";

interface UseEventListenersProps {
  handleBackendResponse: (payload: BackendResponsePayload) => void;
  startNewChat: () => void;
  clearConversation: () => void;
  setCurrentView: (view: AppView) => void;
  setIsDevPanelOpen: (open: boolean) => void;
  setActiveModal: (modal: "help" | null) => void;
  setConversation: React.Dispatch<React.SetStateAction<any[]>>;
  setIsProcessing: (processing: boolean) => void;
  setServerStatus: (status: "checking" | "connected" | "error") => void;
}

export const useEventListeners = ({
  handleBackendResponse,
  startNewChat,
  clearConversation,
  setCurrentView,
  setIsDevPanelOpen,
  setActiveModal,
  setConversation,
  setIsProcessing,
  setServerStatus,
}: UseEventListenersProps) => {
  
  // Listen for backend responses
  useEffect(() => {
    const setupListener = async () => {
      try {
        const unlisten = await listen<BackendResponsePayload>(
          "backend-response",
          (event) => {
            console.log("Received backend-response event:", event.payload);
            handleBackendResponse(event.payload);
          }
        );
        console.log("Backend response listener set up successfully");
        return unlisten;
      } catch (error) {
        console.error("Failed to set up backend response listener:", error);
        return () => {};
      }
    };

    setupListener().then((unlisten) => {
      return () => {
        unlisten();
      };
    });
  }, [handleBackendResponse]);

  // Listen for agent events (thinking, tool calls, etc.)
  useEffect(() => {
    const unlisten = listen<AgentEventTauri>("agent-event", (event) => {
      const { type, payload } = event.payload;
      console.log(`Received agent-event of type: ${type}`, payload);

      const baseMessage = {
        timestamp: Date.now(),
      };

      switch (type) {
        case "thinking":
          setConversation((prev) => [
            ...prev,
            {
              ...baseMessage,
              role: "thinking" as const,
              content: payload.content,
            },
          ]);
          break;

        case "tool_call_request":
          const toolRequestPayload = payload as ToolCallRequestPayload;
          setConversation((prev) => [
            ...prev,
            {
              ...baseMessage,
              role: "tool_call_request" as const,
              content: toolRequestPayload.content || `Calling tool: ${toolRequestPayload.tool_name}`,
              tool_name: toolRequestPayload.tool_name,
              tool_args: toolRequestPayload.tool_args,
            },
          ]);
          break;

        case "tool_call_result":
          const toolResultPayload = payload as ToolCallResultPayload;
          setConversation((prev) => [
            ...prev,
            {
              ...baseMessage,
              role: "tool_call_result" as const,
              content: toolResultPayload.content || "Tool execution completed",
              tool_name: toolResultPayload.tool_name,
              tool_output: toolResultPayload.tool_output,
              success: toolResultPayload.success,
              screenshot_base64: toolResultPayload.screenshot_base64,
            },
          ]);
          break;

        case "screenshot":
          const screenshotPayload = payload as ScreenshotPayload;
          setConversation((prev) => [
            ...prev,
            {
              ...baseMessage,
              role: "assistant" as const,
              content: screenshotPayload.content || "Screenshot captured",
              screenshot_base64: screenshotPayload.screenshot_base64,
            },
          ]);
          break;

        case "generic_content":
          setConversation((prev) => [
            ...prev,
            {
              ...baseMessage,
              role: "assistant" as const,
              content: payload.content,
            },
          ]);
          break;

        default:
          console.warn(`Unknown agent event type: ${type}`);
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setConversation]);

  // Listen for streaming text events
  useEffect(() => {
    const unlisten = listen<StreamingTextEvent>("streaming-text", (event) => {
      const { chunk, message_id } = event.payload;
      console.log(`Received streaming text chunk: "${chunk}" for message_id: ${message_id}`);

      setConversation((prevConversation) => {
        return prevConversation.map((msg) => {
          if (msg.messageId === message_id && msg.isStreaming) {
            return {
              ...msg,
              content: msg.content + chunk,
            };
          }
          return msg;
        });
      });
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setConversation]);

  // Listen for stream start events
  useEffect(() => {
    const unlisten = listen<StreamStartEvent>("stream-start", (event) => {
      const { message_id } = event.payload;
      console.log(`Stream started for message_id: ${message_id}`);

      const streamingMessage = {
        role: "assistant" as const,
        content: "",
        isStreaming: true,
        messageId: message_id,
        timestamp: Date.now(),
      };

      setConversation((prev) => [...prev, streamingMessage]);
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setConversation]);

  // Listen for stream end events
  useEffect(() => {
    const unlisten = listen<StreamEndEvent>("stream-end", (event) => {
      const { message_id, complete_text } = event.payload;
      console.log(`Stream ended for message_id: ${message_id} with complete text length: ${complete_text.length}`);

      setConversation((prevConversation) => {
        return prevConversation.map((msg) => {
          if (msg.messageId === message_id && msg.isStreaming) {
            return {
              ...msg,
              content: complete_text,
              isStreaming: false,
            };
          }
          return msg;
        });
      });

      setIsProcessing(false);
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setConversation, setIsProcessing]);

  // Listen for server status
  useEffect(() => {
    const checkServer = async () => {
      try {
        setServerStatus("checking");
        const isConnected = await invoke<boolean>("check_server_status");
        setServerStatus(isConnected ? "connected" : "error");
      } catch (error) {
        console.error("Server status check failed:", error);
        setServerStatus("error");
      }
    };

    checkServer();
    const interval = setInterval(checkServer, 5000);
    return () => clearInterval(interval);
  }, [setServerStatus]);

  // Menu event listeners
  useEffect(() => {
    const unlisten = listen<string>("settings-requested", (event) => {
      console.log("Settings requested from menu:", event.payload);
      setCurrentView("settings");
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setCurrentView]);

  useEffect(() => {
    const unlisten = listen<string>("devtools-requested", (event) => {
      console.log("DevTools requested from tray menu:", event.payload);
      setCurrentView("devtools");
      setIsDevPanelOpen(true);
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setCurrentView, setIsDevPanelOpen]);

  useEffect(() => {
    const unlisten = listen<string>("help-requested", (event) => {
      console.log("Help requested from menu:", event.payload);
      const helpType = event.payload;

      if (helpType === "shortcuts") {
        setCurrentView("settings");
      } else {
        setActiveModal("help");
      }
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [setCurrentView, setActiveModal]);

  useEffect(() => {
    const unlisten = listen("new-chat-requested", () => {
      console.log("New chat requested from menu");
      startNewChat();
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [startNewChat]);

  useEffect(() => {
    const unlisten = listen("clear-history-requested", () => {
      console.log("Clear history requested from menu");
      clearConversation();
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, [clearConversation]);
};