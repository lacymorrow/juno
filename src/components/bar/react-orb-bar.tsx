import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useDragWindowWithThreshold } from "@/hooks/useDragWindow";
import { EVENTS, UI } from "@/lib/constants.generated";
import { ReactOrb } from "@/components/ui/react-orb";
import { mapToReactOrbConfig, getStatusLabel } from "./bar-state-mapper";
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
const ORB_SIZE = 200;

// The React orb reads hue/hoverIntensity as props and re-initialises its WebGL
// context whenever they change (see react-orb.tsx deps). Quantising the audio-
// reactive intensity to coarse steps bounds how often that re-init fires while
// still giving visible mic/speech feedback.
const INTENSITY_STEP = 0.05;
function quantize(value: number): number {
  return Math.round(value / INTENSITY_STEP) * INTENSITY_STEP;
}

interface ReactOrbBarProps {
  barAppearance?: string;
}

export function ReactOrbBar({ barAppearance: _barAppearance }: ReactOrbBarProps) {
  const [barState, setBarState] = useState<string>(UI.BAR_STATES_DEFAULT);
  const [audioLevel, setAudioLevel] = useState(0);
  const [statusLabel, setStatusLabel] = useState("Ready");
  const [currentError, setCurrentError] = useState<string | null>(null);

  const { resizeWindowIfChanged } = useWindowSize("floating-bar");

  const hasError = barState === UI.BAR_STATES_ERROR;

  // Resize window to fit the orb (a square, same as the ElevenLabs orb bar)
  useEffect(() => {
    resizeWindowIfChanged({ width: ORB_SIZE, height: ORB_SIZE });
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
              setBarState(payload.barState);
              setAudioLevel(payload.audioLevel);
              setStatusLabel(getStatusLabel(payload.barState));
              setCurrentError(payload.currentError);
            }
          }
        );
        if (mounted) unlisten = fn;
        else fn();
      } catch (error) {
        console.error("ReactOrbBar: Failed to setup event listener:", error);
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
        console.error("ReactOrbBar: Interaction failed:", error);
      });
    },
    []
  );

  const handleClick = useCallback(() => {
    sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sendInteraction]);

  // Map the current bar state to hue/intensity/hover, then modulate intensity
  // with the live audio level for listening (mic) and talking (speech) states.
  const { hue, hoverIntensity, forceHoverState } = useMemo(() => {
    const config = mapToReactOrbConfig(barState);
    let intensity = config.hoverIntensity;
    if (config.reactive !== null) {
      const level = Math.min(1, Math.max(0, audioLevel));
      intensity = quantize(config.hoverIntensity + level * 0.4);
    }
    return {
      hue: config.hue,
      hoverIntensity: intensity,
      forceHoverState: config.forceHoverState,
    };
  }, [barState, audioLevel]);

  const dragHandlers = useDragWindowWithThreshold();

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full select-none cursor-grab active:cursor-grabbing"
      onClick={handleClick}
      {...dragHandlers}
    >
      <ReactOrb
        hue={hue}
        hoverIntensity={hoverIntensity}
        forceHoverState={forceHoverState}
        rotateOnHover={true}
        className="w-full h-full"
      />
      {statusLabel && (
        <div className="absolute bottom-2 text-[10px] text-white/70 text-center truncate max-w-full px-2 pointer-events-none">
          {hasError && currentError ? currentError : statusLabel}
        </div>
      )}
    </div>
  );
}
