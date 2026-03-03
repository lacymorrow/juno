import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { EVENTS, UI } from "@/lib/constants.generated";
import { Persona } from "@/components/ai-elements/persona";
import { mapToPersonaState, getStatusLabel } from "./bar-state-mapper";
import { useWindowSize } from "@/hooks/useWindowSize";

interface BarStateData {
  barState: string;
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
  data: Record<string, unknown> | null;
  timestamp: number;
}

const COMPONENT_ID = "floating-bar";
const PERSONA_SIZE = 200;

interface PersonaBarProps {
  barAppearance?: string;
}

export function PersonaBar({ barAppearance: _barAppearance }: PersonaBarProps) {
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

  const { resizeWindowIfChanged } = useWindowSize("floating-bar");

  // Resize window to fit the persona
  useEffect(() => {
    resizeWindowIfChanged({ width: PERSONA_SIZE, height: PERSONA_SIZE });
  }, [resizeWindowIfChanged]);

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
        console.error("PersonaBar: Failed to setup event listener:", error);
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
    (interactionType: string, data?: Record<string, unknown>) => {
      const interaction: UIInteractionEvent = {
        element_id: COMPONENT_ID,
        interaction_type: interactionType,
        data: data ?? null,
        timestamp: Date.now(),
      };
      invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction,
      }).catch((error) => {
        console.error("PersonaBar: Interaction failed:", error);
      });
    },
    []
  );

  const handleClick = useCallback(() => {
    sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sendInteraction]);

  // === DERIVED STATE ===
  const personaState = mapToPersonaState(barState.barState);
  const statusLabel = getStatusLabel(barState.barState);
  const hasError = barState.barState === UI.BAR_STATES_ERROR;

  return (
    <div
      className="relative select-none cursor-pointer"
      style={{ width: PERSONA_SIZE, height: PERSONA_SIZE }}
      onClick={handleClick}
      data-tauri-drag-region
    >
      <Persona
        state={personaState}
        variant="obsidian"
        className="!w-[200px] !h-[200px]"
      />
      {statusLabel && (
        <div className="absolute bottom-2 left-0 right-0 text-[10px] text-white/70 text-center truncate px-2 pointer-events-none">
          {hasError && barState.currentError
            ? barState.currentError
            : statusLabel}
        </div>
      )}
    </div>
  );
}
