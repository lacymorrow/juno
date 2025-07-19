import { useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { stopTTS } from "@/lib/ttsService";
import { safeUnlistenAll } from "@/lib/tauri-event-utils";

import { EVENTS, COMMANDS } from '../lib/constants.generated';
// Simple type for event payloads - keep it minimal
type BackendEventPayload = {
  // Common fields
  stopType?: 'normal' | 'force' | 'error';
  message_id?: string;
  chunk?: string;
  complete_text?: string;
  agent_state?: string;
  error_message?: string;
  content?: string;
  tool_name?: string;
  success?: boolean;
  audio_base64?: string;
  
  // Backend response
  query?: string;
  response?: {
    text: string;
    spoken_text?: string;
    audio_base64?: string;
    agent_state: string;
    screenshot_base64?: string;
  };
  
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
  // Thread-safe streaming message storage with proper synchronization
  const streamingMessages = useRef<Map<string, string>>(new Map());
  const streamingLock = useRef<Map<string, boolean>>(new Map());

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

        case EVENTS.AGENT_STOP:
          console.log("Agent stop event:", payload.stopType);
          await stopCurrentAudio();
          await stopTTS((msg) => console.log(`[TTS] ${msg}`));
          setIsProcessing(false);
          break;

        case EVENTS.TTS_STOP_REQUESTED:
          console.log("Stopping TTS...");
          await stopCurrentAudio();
          await stopTTS((msg) => console.log(`[TTS] ${msg}`));
          break;

        // Audio events
        case EVENTS.TTS_AUDIO_READY:
          if (payload.audio_base64) {
            await playAudioFromBase64(payload.audio_base64);
          }
          break;

        // Streaming events - real-time streaming implementation
        case EVENTS.STREAMING_STREAM_START:
          console.log("Stream started:", payload.message_id);
          if (payload.message_id) {
            // Ensure atomic initialization
            streamingLock.current.set(payload.message_id, true);
            streamingMessages.current.set(payload.message_id, "");
            addOrUpdateStreamingMessage(payload.message_id, "", false);
            streamingLock.current.set(payload.message_id, false);
          }
          break;

        case EVENTS.STREAMING_TEXT_STREAM:
          if (payload.message_id && payload.chunk) {
            // Wait if another operation is in progress
            while (streamingLock.current.get(payload.message_id)) {
              // Yield to prevent blocking
              await new Promise(resolve => setTimeout(resolve, 0));
            }
            
            streamingLock.current.set(payload.message_id, true);
            try {
              const existing = streamingMessages.current.get(payload.message_id) || "";
              const newText = existing + payload.chunk;
              streamingMessages.current.set(payload.message_id, newText);
              addOrUpdateStreamingMessage(payload.message_id, newText, false);
            } finally {
              streamingLock.current.set(payload.message_id, false);
            }
          }
          break;

        case EVENTS.STREAMING_STREAM_END:
          if (payload.message_id) {
            // Wait for any pending operations
            while (streamingLock.current.get(payload.message_id)) {
              await new Promise(resolve => setTimeout(resolve, 0));
            }
            
            streamingLock.current.set(payload.message_id, true);
            try {
              const finalText = payload.complete_text || streamingMessages.current.get(payload.message_id) || "";
              if (finalText) {
                addOrUpdateStreamingMessage(payload.message_id, finalText, true);
              }
              streamingMessages.current.delete(payload.message_id);
              streamingLock.current.delete(payload.message_id);
              
              const agentState = payload.agent_state;
              console.log(`[Event] agent-stream-end with state: ${agentState}`);
              
              // Handle all completion states (including "Offline" for network errors)
              if (agentState === "Finished" || agentState === "Failed" || agentState === "Cancelled" || agentState === "Offline") {
                console.log(`[Event] Setting isProcessing to false due to agent state: ${agentState}`);
                setIsProcessing(false);
              } else {
                console.log(`[Event] Keeping isProcessing true - unexpected agent state: ${agentState}`);
              }
            } finally {
              streamingLock.current.delete(payload.message_id);
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
        case EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED:
          if (payload.content) {
            // User messages are handled elsewhere, just log for now
            console.log("User message submitted:", payload.content);
          }
          break;

        // Agent events (generic)
        case EVENTS.AGENT_EVENT:
          console.log("Agent event received:", payload);
          // These might contain thinking, tool calls, etc.
          if (payload.type === "thinking" && payload.payload?.content) {
            // Could show thinking process to user
            console.log("Agent thinking:", payload.payload.content);
          }
          break;

        // Tool usage events
        case EVENTS.TOOLS_USAGE:
          console.log("Tool usage event:", payload);
          // Could show tool execution feedback
          if (payload.tool_name && payload.success !== undefined) {
            const status = payload.success ? "✅" : "❌";
            console.log(`Tool ${payload.tool_name}: ${status}`);
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
          await invoke(COMMANDS.UTILS_CHECK_SERVER_STATUS);
        } catch (error) {
          console.warn("Server status check failed:", error);
        }
      }

      // Event types we want to listen to
      const eventTypes = [
        "backend-response",
        "agent-stop", // Consolidated stop event
        "tts-audio-ready",
        "tts-stop-requested",
        "agent-stream-start",
        "agent-text-stream", // Fixed event name to match backend
        "agent-stream-end",
        "agent-error",
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

    // Cleanup all subscriptions safely
    return () => {
      safeUnlistenAll(eventSubscriptions);
      eventSubscriptions.length = 0;
      
      // Clean up any remaining locks
      streamingLock.current.clear();
      streamingMessages.current.clear();
    };
  }, [handleBackendEvent]);

  return {
    // Hook is ready when listeners are set up
    isListening: true,
  };
};