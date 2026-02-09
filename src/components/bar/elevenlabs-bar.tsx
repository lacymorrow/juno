import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { EVENTS, UI } from "@/lib/constants.generated";
import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { VoiceButton } from "@/components/ui/voice-button";
import {
  mapToAgentState,
  mapToVoiceButtonState,
  getStatusLabel,
} from "./elevenlabs-state-mapper";

// === TYPES ===

type UIState =
  | typeof UI.BAR_STATES_DEFAULT
  | typeof UI.BAR_STATES_EXPANDING
  | typeof UI.BAR_STATES_INPUT
  | typeof UI.BAR_STATES_SHRINKING
  | typeof UI.BAR_STATES_SUBMITTING
  | typeof UI.BAR_STATES_LOADING
  | typeof UI.BAR_STATES_FINISHING
  | typeof UI.BAR_STATES_SUCCESS
  | typeof UI.BAR_STATES_LISTENING
  | typeof UI.BAR_STATES_ERROR
  | typeof UI.BAR_STATES_TRANSCRIBING
  | typeof UI.BAR_STATES_SPEAKING
  | typeof UI.BAR_STATES_DICTATING
  | typeof UI.BAR_STATES_DICTATION_READY
  | typeof UI.BAR_STATES_ALWAYS_LISTENING
  | typeof UI.BAR_STATES_AGENT_RESPONDING;

interface BarStateData {
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  agentState: string | null;
}

interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

const COMPONENT_ID = "elevenlabs-bar";

interface ElevenLabsBarProps {
  barAppearance?: string;
}

export function ElevenLabsBar({ barAppearance: _barAppearance }: ElevenLabsBarProps) {
  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: UI.VOICE_MODES_IDLE,
    agentState: null,
  });

  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // === BACKEND EVENT LISTENER ===
  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const fn = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            if (!mounted) return;
            const payload = event.payload;
            if (payload && typeof payload.barState === "string") {
              setBarState(payload);
            }
          }
        );
        if (mounted) unlisten = fn;
        else fn();
      } catch (error) {
        console.error("ElevenLabsBar: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // === INTERACTION HANDLER ===
  const sendInteraction = useCallback(
    (interactionType: string, data?: Record<string, any>) => {
      const event: UIInteractionEvent = {
        element_id: COMPONENT_ID,
        interaction_type: interactionType,
        data: data ?? null,
        timestamp: Date.now(),
      };
      invoke("ui_handle_interaction", { event }).catch((error) => {
        console.error("ElevenLabsBar: Interaction failed:", error);
      });
    },
    []
  );

  const handleVoiceButtonPress = useCallback(() => {
    sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sendInteraction]);

  // === DERIVED STATE ===
  const agentState = mapToAgentState(barState.barState);
  const voiceButtonState = mapToVoiceButtonState(barState.barState);
  const statusLabel = getStatusLabel(barState.barState);
  const hasError = barState.barState === UI.BAR_STATES_ERROR;

  return (
    <div className="flex flex-col items-center justify-center w-full h-full gap-2 p-2 select-none">
      {/* Bar Visualizer — uses demo mode since we don't have direct mic MediaStream */}
      <BarVisualizer
        state={agentState}
        barCount={15}
        demo
        minHeight={10}
        maxHeight={100}
        className="w-full h-20 rounded-xl bg-transparent"
      />

      {/* Status label */}
      {statusLabel && (
        <div className="text-xs text-muted-foreground text-center truncate max-w-full px-2">
          {hasError && barState.currentError
            ? barState.currentError
            : barState.transcriptionText || statusLabel}
        </div>
      )}

      {/* Voice button */}
      <VoiceButton
        state={voiceButtonState}
        onPress={handleVoiceButtonPress}
        label="Juno"
        trailing="Click to talk"
        size="default"
        variant="outline"
        className="w-full max-w-[200px]"
      />
    </div>
  );
}
