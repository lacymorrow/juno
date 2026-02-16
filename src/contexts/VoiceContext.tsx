import { listen } from "@tauri-apps/api/event";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { EVENTS, UI } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

export interface VoiceState {
  mode:
    | typeof UI.VOICE_MODES_DICTATION
    | typeof UI.VOICE_MODES_AGENT
    | typeof UI.VOICE_MODES_IDLE;
  isListening: boolean;
  isTranscribing: boolean;
  isSpeaking: boolean;
  transcriptionText?: string;
  error?: string;
  audioLevel: number;
}

export interface AgentState {
  status:
    | typeof UI.AGENT_STATUS_IDLE
    | typeof UI.AGENT_STATUS_LISTENING
    | typeof UI.AGENT_STATUS_THINKING
    | typeof UI.AGENT_STATUS_RESPONDING
    | typeof UI.AGENT_STATUS_ERROR;
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
  /**
   * Set isSpeaking state for TTS playback.
   * Should be called from audio playback logic:
   *   setIsSpeaking(true) when audio starts
   *   setIsSpeaking(false) when audio ends
   */
  setIsSpeaking: (isSpeaking: boolean) => void;
}

// Removed unused initial state constants since they were duplicates of the useState defaults

const VoiceContext = createContext<VoiceContextType | undefined>(undefined);

interface VoiceProviderProps {
  children: ReactNode;
}

export function VoiceProvider({ children }: VoiceProviderProps) {
  const [voiceState, setVoiceState] = useState<VoiceState>({
    mode: UI.VOICE_MODES_IDLE,
    isListening: false,
    isTranscribing: false,
    isSpeaking: false,
    audioLevel: 0,
  });

  const [agentState, setAgentState] = useState<AgentState>({
    status: UI.AGENT_STATUS_IDLE,
  });

  const [recentMessages, setRecentMessages] = useState<ChatMessage[]>([]);

  useEffect(() => {
    let mounted = true;
    const unlistenCallbacks: (() => void)[] = [];

    const addListener = async <T,>(eventName: string, handler: (event: { payload: T }) => void) => {
      try {
        const unlisten = await listen<T>(eventName, (event) => {
          if (mounted) handler(event);
        });
        if (mounted) {
          unlistenCallbacks.push(unlisten);
        } else {
          safeCleanupEventListener(unlisten);
        }
      } catch (error) {
        console.error(`Failed to setup listener for ${eventName}:`, error);
      }
    };

    const setupListeners = async () => {
      // Voice transcription events
      await addListener(EVENTS.VOICE_TRANSCRIPTION_PARTIAL_RESULT, (event) => {
        const text = event.payload as string;
        setVoiceState((prev) => ({
          ...prev,
          isTranscribing: true,
          transcriptionText: text,
        }));
      });

      await addListener(EVENTS.VOICE_TRANSCRIPTION_FINAL_RESULT, (event) => {
        const text = event.payload as string;
        setVoiceState((prev) => ({
          ...prev,
          isTranscribing: false,
          transcriptionText: text,
        }));

        // Add user message to recent messages
        setRecentMessages((prev) => [
          ...prev.slice(-3), // Keep last 3 + new = 4 messages
          {
            role: "user",
            content: text,
            timestamp: Date.now(),
          },
        ]);
      });

      // Audio level updates
      await addListener<number>("audio-level", (event) => {
        setVoiceState((prev) => ({
          ...prev,
          audioLevel: event.payload,
        }));
      });

      // Error handling - using voice transcription error event
      await addListener<string>(EVENTS.VOICE_TRANSCRIPTION_ERROR, (event) => {
        const error = event.payload;
        setVoiceState((prev) => ({
          ...prev,
          error,
          isListening: false,
          isTranscribing: false,
        }));
        setAgentState((prev) => ({
          ...prev,
          status: UI.AGENT_STATUS_ERROR,
          error,
        }));
      });

      // Dictation active flag — simple boolean, most reliable signal
      await addListener<boolean>(EVENTS.DICTATION_ACTIVE, (event) => {
        const isActive = event.payload;
        setVoiceState((prev) => ({
          ...prev,
          isListening: isActive,
          mode: isActive ? UI.VOICE_MODES_DICTATION : UI.VOICE_MODES_IDLE,
          ...(isActive ? { error: undefined } : {}),
        }));
      });

      // Dictation state changes — handles both payload formats:
      // 1. ui_commands.rs: { new_state: "dictation" | "idle" | "always_listening" } (String)
      // 2. dictation_state_manager.rs: { new_state: "Idle" | "Starting" | { Active: ... } } (Rust enum)
      await addListener<{
        previous_state: unknown;
        new_state: unknown;
        timestamp: number;
        reason: string;
        component: string;
      }>(EVENTS.DICTATION_STATE_CHANGED, (event) => {
        const { new_state } = event.payload;

        // String format from ui_commands.rs (lowercase voice mode names)
        if (new_state === "dictation") {
          setVoiceState((prev) => ({
            ...prev,
            isListening: true,
            mode: UI.VOICE_MODES_DICTATION,
            error: undefined,
          }));
        } else if (new_state === "idle") {
          setVoiceState((prev) => ({
            ...prev,
            isListening: false,
            mode: UI.VOICE_MODES_IDLE,
          }));
        } else if (new_state === "agent") {
          setVoiceState((prev) => ({
            ...prev,
            isListening: true,
            mode: UI.VOICE_MODES_AGENT,
            error: undefined,
          }));
        } else if (new_state === "always_listening") {
          setVoiceState((prev) => ({
            ...prev,
            isListening: true,
            error: undefined,
          }));
        // Rust enum format from dictation_state_manager.rs (capitalized variants)
        } else if (typeof new_state === "object" && new_state !== null && "Active" in new_state) {
          setVoiceState((prev) => ({
            ...prev,
            isListening: true,
            mode: UI.VOICE_MODES_DICTATION,
            error: undefined,
          }));
        } else if (new_state === "Idle" || new_state === "Stopping" || new_state === "ForceResetting") {
          setVoiceState((prev) => ({
            ...prev,
            isListening: false,
            mode: UI.VOICE_MODES_IDLE,
          }));
        } else if (new_state === "Starting") {
          setVoiceState((prev) => ({
            ...prev,
            mode: UI.VOICE_MODES_DICTATION,
          }));
        } else if (typeof new_state === "object" && new_state !== null && "Error" in new_state) {
          const errorObj = new_state as { Error: { message: string } };
          setVoiceState((prev) => ({
            ...prev,
            isListening: false,
            error: errorObj.Error.message,
          }));
        }
      });

      // Fallback: direct dictation start/stop events
      await addListener(EVENTS.DICTATION_STARTED, () => {
        setVoiceState((prev) => ({
          ...prev,
          isListening: true,
          mode: UI.VOICE_MODES_DICTATION,
        }));
      });

      await addListener(EVENTS.VOICE_TRANSCRIPTION_DICTATION_STOPPED, () => {
        setVoiceState((prev) => ({
          ...prev,
          isListening: false,
          mode: UI.VOICE_MODES_IDLE,
        }));
      });

      // Always-listening mode
      await addListener(EVENTS.ALWAYS_LISTENING_STARTED, () => {
        setVoiceState((prev) => ({
          ...prev,
          isListening: true,
        }));
      });

      await addListener(EVENTS.ALWAYS_LISTENING_STOPPED, () => {
        setVoiceState((prev) => ({
          ...prev,
          isListening: false,
          mode: UI.VOICE_MODES_IDLE,
        }));
      });

      // Agent-specific events
      await addListener(EVENTS.AGENT_ACTIVE, () => {
        setAgentState((prev) => ({
          ...prev,
          status: UI.AGENT_STATUS_LISTENING,
        }));
      });

      await addListener(EVENTS.AGENT_THOUGHT_PROCESS, () => {
        setAgentState((prev) => ({
          ...prev,
          status: UI.AGENT_STATUS_THINKING,
        }));
      });

      // AI response streaming
      await addListener(EVENTS.STREAMING_TEXT_STREAM, (event) => {
        const chunk = event.payload as { chunk: string; message_id: string };
        setAgentState((prev) => ({
          ...prev,
          status: UI.AGENT_STATUS_RESPONDING,
          currentResponse: (prev.currentResponse || "") + chunk.chunk,
        }));
      });

      await addListener(EVENTS.STREAMING_STREAM_END, (event) => {
        const data = event.payload as {
          message_id: string;
          complete_text: string;
        };

        // Add assistant message to recent messages
        setRecentMessages((prev) => [
          ...prev.slice(-3), // Keep last 3 + new = 4 messages
          {
            role: "assistant",
            content: data.complete_text,
            timestamp: Date.now(),
          },
        ]);

        setAgentState((prev) => ({
          ...prev,
          currentResponse: undefined,
          status: UI.AGENT_STATUS_IDLE,
        }));
      });
    };

    setupListeners();

    return () => {
      mounted = false;
      unlistenCallbacks.forEach(safeCleanupEventListener);
    };
  }, []); // Empty deps — listeners set up once on mount

  // Clear transcription text after a delay when not active
  useEffect(() => {
    let timer: NodeJS.Timeout | undefined;
    
    if (
      !voiceState.isListening &&
      !voiceState.isTranscribing &&
      voiceState.transcriptionText
    ) {
      timer = setTimeout(() => {
        setVoiceState((prev) => ({ ...prev, transcriptionText: undefined }));
      }, 3000);
    }
    
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
    };
  }, [
    voiceState.isListening,
    voiceState.isTranscribing,
    voiceState.transcriptionText,
  ]);

  // Context methods — useCallback to stabilize references for useMemo
  const addMessage = useCallback((message: Omit<ChatMessage, "timestamp">) => {
    setRecentMessages((prev) => [
      ...prev.slice(-3), // Keep last 3 + new = 4 messages
      { ...message, timestamp: Date.now() },
    ]);
  }, []);

  const clearError = useCallback(() => {
    setVoiceState((prev) => ({ ...prev, error: undefined }));
    setAgentState((prev) => ({ ...prev, error: undefined }));
  }, []);

  const resetTranscription = useCallback(() => {
    setVoiceState((prev) => ({ ...prev, transcriptionText: undefined }));
  }, []);

  const setIsSpeaking = useCallback((isSpeaking: boolean) => {
    setVoiceState((prev) => ({ ...prev, isSpeaking }));
  }, []);

  const contextValue = useMemo<VoiceContextType>(() => ({
    voiceState,
    agentState,
    recentMessages,
    addMessage,
    clearError,
    resetTranscription,
    setIsSpeaking,
  }), [voiceState, agentState, recentMessages, addMessage, clearError, resetTranscription, setIsSpeaking]);

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
