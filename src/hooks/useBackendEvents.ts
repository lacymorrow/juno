import { useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { stopTTS } from "@/lib/ttsService";

// Simplified type definitions
type BackendEventPayload = {
  query?: string;
  response?: {
    text: string;
    spoken_text?: string;
    audio_base64?: string;
    agent_state: string;
    screenshot_base64?: string;
  };
  audio_base64?: string;
  chunk?: string;
  message_id?: string;
  agent_state?: string;
  complete_text?: string;
  error_message?: string;
  content?: string;
  tool_name?: string;
  [key: string]: any;
};

interface UseBackendEventsProps {
  addSystemMessage: (message: string) => void;
  addAssistantMessage: (message: string) => void;
  addOrUpdateStreamingMessage: (messageId: string, content: string, isComplete?: boolean) => void;
  playAudioFromBase64: (base64: string) => Promise<void>;
  stopCurrentAudio: () => Promise<void>;
  setIsProcessing: (processing: boolean) => void;
}

export const useBackendEvents = ({
  addSystemMessage,
  addAssistantMessage,
  addOrUpdateStreamingMessage,
  playAudioFromBase64,
  stopCurrentAudio,
  setIsProcessing,
}: UseBackendEventsProps) => {
  const hasCheckedServer = useRef(false);
  const streamingMessages = useRef<Map<string, string>>(new Map());

  // Consolidated event handler
  const handleBackendEvent = useCallback(async (
    eventType: string, 
    payload: BackendEventPayload
  ) => {
    console.log(`[Event] ${eventType}:`, payload);

    try {
      switch (eventType) {
        // Agent lifecycle events
        case "backend-response":
          if (payload.response) {
            addAssistantMessage(payload.response.text);
            
            if (payload.response.audio_base64) {
              await playAudioFromBase64(payload.response.audio_base64);
            }
          }
          setIsProcessing(false);
          break;

        case "agent-stopping":
        case "tts-stop-requested":
        case "agent-stop-all":
          console.log("Stopping operations...");
          await stopCurrentAudio();
          await stopTTS((msg) => console.log(`[TTS] ${msg}`));
          setIsProcessing(false);
          break;

        // Audio events
        case "tts-audio-ready":
          if (payload.audio_base64) {
            await playAudioFromBase64(payload.audio_base64);
          }
          break;

        // Streaming events - real-time streaming implementation
        case "agent-stream-start":
          console.log("Stream started:", payload.message_id);
          if (payload.message_id) {
            // Initialize streaming message with empty content
            streamingMessages.current.set(payload.message_id, "");
            addOrUpdateStreamingMessage(payload.message_id, "", false);
          }
          break;

        case "agent-text-stream":
          // Real-time text streaming (fixed event name)
          if (payload.message_id && payload.chunk) {
            const existing = streamingMessages.current.get(payload.message_id) || "";
            const newText = existing + payload.chunk;
            streamingMessages.current.set(payload.message_id, newText);
            
            // Update the streaming message in real-time
            addOrUpdateStreamingMessage(payload.message_id, newText, false);
          }
          break;

        case "agent-stream-end":
          if (payload.message_id) {
            const finalText = payload.complete_text || streamingMessages.current.get(payload.message_id) || "";
            if (finalText) {
              // Mark streaming as complete with final text
              addOrUpdateStreamingMessage(payload.message_id, finalText, true);
            }
            streamingMessages.current.delete(payload.message_id);
            
            // Only set isProcessing to false if agent has actually finished
            // Check the agent_state field to determine if processing is complete
            const agentState = payload.agent_state;
            console.log(`[Event] agent-stream-end with state: ${agentState}`);
            
            // Handle all completion states (including "Offline" for network errors)
            if (agentState === "Finished" || agentState === "Failed" || agentState === "Cancelled" || agentState === "Offline") {
              console.log(`[Event] Setting isProcessing to false due to agent state: ${agentState}`);
              setIsProcessing(false);
            } else {
              console.log(`[Event] Keeping isProcessing true - unexpected agent state: ${agentState}`);
            }
          }
          break;

        // Error handling
        case "agent-error":
          const errorText = payload.error_message || "An error occurred";
          addSystemMessage(errorText);
          setIsProcessing(false);
          toast.error(errorText);
          break;

        // User messages
        case "user-message-submitted":
          if (payload.content) {
            // User messages are handled elsewhere, just log for now
            console.log("User message submitted:", payload.content);
          }
          break;

        // Agent events (generic)
        case "agent-event":
          console.log("Agent event received:", payload);
          // These might contain thinking, tool calls, etc.
          if (payload.type === "thinking" && payload.payload?.content) {
            // Could show thinking process to user
            console.log("Agent thinking:", payload.payload.content);
          }
          break;

        // Tool usage events
        case "tool-usage":
          console.log("Tool usage event:", payload);
          // Could show tool execution feedback
          if (payload.tool && payload.success !== undefined) {
            const status = payload.success ? "✅" : "❌";
            console.log(`Tool ${payload.tool}: ${status}`);
          }
          break;

        default:
          console.log(`[Event] Unhandled event type: ${eventType}`);
      }
    } catch (error) {
      console.error(`[Event] Error handling ${eventType}:`, error);
    }
  }, [addSystemMessage, addAssistantMessage, addOrUpdateStreamingMessage, playAudioFromBase64, stopCurrentAudio, setIsProcessing]);

  // Single useEffect for all backend events - clean and simple
  useEffect(() => {
    const eventSubscriptions: Array<() => void> = [];
    
    const setupEventListeners = async () => {
      // Check server status once
      if (!hasCheckedServer.current) {
        hasCheckedServer.current = true;
        try {
          await invoke("check_server_status");
        } catch (error) {
          console.warn("Server status check failed:", error);
        }
      }

      // Event types we want to listen to
      const eventTypes = [
        "backend-response",
        "agent-stopping", 
        "tts-audio-ready",
        "tts-stop-requested",
        "agent-stream-start",
        "agent-text-stream", // Fixed event name to match backend
        "agent-stream-end",
        "agent-error",
        "agent-stop-all",
        "agent-event", // Generic agent events
        "tool-usage", // Tool execution events
        "user-message-submitted",
      ];

      // Register all event listeners
      for (const eventType of eventTypes) {
        try {
          const unlisten = await listen(eventType, (event: any) => {
            handleBackendEvent(eventType, event.payload || {});
          });
          eventSubscriptions.push(unlisten);
        } catch (error) {
          console.error(`Failed to setup listener for ${eventType}:`, error);
        }
      }
    };

    setupEventListeners();

    // Cleanup all subscriptions
    return () => {
      eventSubscriptions.forEach(unlisten => {
        try {
          unlisten();
        } catch (error) {
          console.error("Error cleaning up event listener:", error);
        }
      });
      eventSubscriptions.length = 0;
    };
  }, [handleBackendEvent]);

  return {
    // Hook is ready when listeners are set up
    isListening: true,
  };
};