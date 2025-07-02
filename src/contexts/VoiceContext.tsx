import { listen } from "@tauri-apps/api/event";
import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

export interface VoiceState {
  mode: "dictation" | "agent" | "idle";
  isListening: boolean;
  isTranscribing: boolean;
  isSpeaking: boolean;
  transcriptionText?: string;
  error?: string;
  audioLevel: number;
}

export interface AgentState {
  status: "idle" | "listening" | "thinking" | "responding" | "error";
  currentResponse?: string;
  error?: string;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  isStreaming?: boolean;
}

interface VoiceContextType {
  voiceState: VoiceState;
  agentState: AgentState;
  recentMessages: ChatMessage[];
  addMessage: (message: Omit<ChatMessage, "timestamp">) => void;
  clearError: () => void;
  resetTranscription: () => void;
}

const initialVoiceState: VoiceState = {
  mode: "idle",
  isListening: false,
  isTranscribing: false,
  isSpeaking: false,
  audioLevel: 0,
};

const initialAgentState: AgentState = {
  status: "idle",
};

const VoiceContext = createContext<VoiceContextType | undefined>(undefined);

interface VoiceProviderProps {
  children: ReactNode;
}

export function VoiceProvider({ children }: VoiceProviderProps) {
  const [voiceState, setVoiceState] = useState<VoiceState>(initialVoiceState);
  const [agentState, setAgentState] = useState<AgentState>(initialAgentState);
  const [recentMessages, setRecentMessages] = useState<ChatMessage[]>([]);

  // Set up all voice-related event listeners in one place
  useEffect(() => {
    let unlistenCallbacks: (() => void)[] = [];

    const setupListeners = async () => {
      // Voice mode events
      unlistenCallbacks.push(
        await listen("dictation-active", (event) => {
          const isActive = event.payload as boolean;
          setVoiceState((prev) => ({
            ...prev,
            mode: isActive ? "dictation" : "idle",
            isListening: isActive,
          }));
        })
      );

      unlistenCallbacks.push(
        await listen("app-dictation-started", () => {
          setVoiceState((prev) => ({
            ...prev,
            mode: "agent",
            isListening: true,
          }));
          setAgentState((prev) => ({ ...prev, status: "listening" }));
        })
      );

      unlistenCallbacks.push(
        await listen("dictation-active", (event) => {
          const isActive = event.payload as boolean;
          setVoiceState((prev) => ({
            ...prev,
            isListening: isActive,
          }));
        })
      );

      // Transcription events
      unlistenCallbacks.push(
        await listen("dictation-transcription-partial", (event) => {
          const text = event.payload as string;
          setVoiceState((prev) => ({
            ...prev,
            isTranscribing: true,
            transcriptionText: text,
          }));
        })
      );

      unlistenCallbacks.push(
        await listen("dictation-transcription-final", (event) => {
          const text = event.payload as string;
          setVoiceState((prev) => ({
            ...prev,
            isTranscribing: false,
            transcriptionText: text,
          }));

          // Add user message to recent messages
          setRecentMessages((prev) => [
            ...prev.slice(-4), // Keep last 4 messages
            {
              role: "user",
              content: text,
              timestamp: Date.now(),
            },
          ]);
        })
      );

      // TTS events
      unlistenCallbacks.push(
        await listen("tts-started", () => {
          setVoiceState((prev) => ({
            ...prev,
            isSpeaking: true,
          }));
        })
      );

      unlistenCallbacks.push(
        await listen("tts-finished", () => {
          setVoiceState((prev) => ({
            ...prev,
            isSpeaking: false,
          }));
        })
      );

      // Audio level updates
      unlistenCallbacks.push(
        await listen<number>("audio-level", (event) => {
          setVoiceState((prev) => ({
            ...prev,
            audioLevel: event.payload,
          }));
        })
      );

      // Error handling
      unlistenCallbacks.push(
        await listen<string>("voice-error", (event) => {
          const error = event.payload;
          setVoiceState((prev) => ({
            ...prev,
            error,
            isListening: false,
            isTranscribing: false,
          }));
          setAgentState((prev) => ({ ...prev, status: "error", error }));
        })
      );

      // Agent-specific events
      unlistenCallbacks.push(
        await listen("agent-started", () => {
          setAgentState((prev) => ({ ...prev, status: "listening" }));
        })
      );

      unlistenCallbacks.push(
        await listen("agent-thinking", () => {
          setAgentState((prev) => ({ ...prev, status: "thinking" }));
        })
      );

      unlistenCallbacks.push(
        await listen("agent-responding", () => {
          setAgentState((prev) => ({ ...prev, status: "responding" }));
        })
      );

      // AI response streaming
      unlistenCallbacks.push(
        await listen("streaming-text", (event) => {
          const chunk = event.payload as { chunk: string; message_id: string };
          setAgentState((prev) => ({
            ...prev,
            currentResponse: (prev.currentResponse || "") + chunk.chunk,
          }));
        })
      );

      unlistenCallbacks.push(
        await listen("stream-end", (event) => {
          const data = event.payload as {
            message_id: string;
            complete_text: string;
          };

          // Add assistant message to recent messages
          setRecentMessages((prev) => [
            ...prev.slice(-4),
            {
              role: "assistant",
              content: data.complete_text,
              timestamp: Date.now(),
            },
          ]);

          setAgentState((prev) => ({
            ...prev,
            currentResponse: undefined,
            status: "idle",
          }));
        })
      );
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenCallbacks.forEach((unlisten) => unlisten());
    };
  }, []);

  // Clear transcription text after a delay when not active
  useEffect(() => {
    if (
      !voiceState.isListening &&
      !voiceState.isTranscribing &&
      voiceState.transcriptionText
    ) {
      const timer = setTimeout(() => {
        setVoiceState((prev) => ({ ...prev, transcriptionText: undefined }));
      }, 3000);
      return () => clearTimeout(timer);
    }
  }, [
    voiceState.isListening,
    voiceState.isTranscribing,
    voiceState.transcriptionText,
  ]);

  // Context methods
  const addMessage = (message: Omit<ChatMessage, "timestamp">) => {
    setRecentMessages((prev) => [
      ...prev.slice(-4),
      { ...message, timestamp: Date.now() },
    ]);
  };

  const clearError = () => {
    setVoiceState((prev) => ({ ...prev, error: undefined }));
    setAgentState((prev) => ({ ...prev, error: undefined }));
  };

  const resetTranscription = () => {
    setVoiceState((prev) => ({ ...prev, transcriptionText: undefined }));
  };

  const contextValue: VoiceContextType = {
    voiceState,
    agentState,
    recentMessages,
    addMessage,
    clearError,
    resetTranscription,
  };

  return (
    <VoiceContext.Provider value={contextValue}>
      {children}
    </VoiceContext.Provider>
  );
}

export function useVoice() {
  const context = useContext(VoiceContext);
  if (context === undefined) {
    throw new Error("useVoice must be used within a VoiceProvider");
  }
  return context;
}

// Backward compatibility - export individual hooks
export function useVoiceState() {
  const { voiceState } = useVoice();
  return voiceState;
}

export function useAgentState() {
  const { agentState } = useVoice();
  return agentState;
}

export function useRecentMessages() {
  const { recentMessages, addMessage } = useVoice();
  return { recentMessages, addMessage };
}
