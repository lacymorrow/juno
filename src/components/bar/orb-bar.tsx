import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useDragWindowWithThreshold } from "@/hooks/useDragWindow";
import { EVENTS, UI } from "@/lib/constants.generated";
import type { AgentState } from "@/components/ui/orb";
import { Orb } from "@/components/ui/orb";
import { mapToOrbState, getStatusLabel } from "./bar-state-mapper";
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

// State-driven color palettes — each pair is [primary, secondary]
const STATE_COLORS = {
  idle: ["#6366F1", "#8B5CF6"] as [string, string],
  listening: ["#3B82F6", "#6366F1"] as [string, string],
  thinking: ["#F59E0B", "#EF4444"] as [string, string],
  talking: ["#10B981", "#06B6D4"] as [string, string],
  error: ["#EF4444", "#DC2626"] as [string, string],
};

function getColorsForOrbState(
  orbState: AgentState,
  isError: boolean
): [string, string] {
  if (isError) return STATE_COLORS.error;
  if (orbState === "listening") return STATE_COLORS.listening;
  if (orbState === "thinking") return STATE_COLORS.thinking;
  if (orbState === "talking") return STATE_COLORS.talking;
  return STATE_COLORS.idle;
}

interface OrbBarProps {
  barAppearance?: string;
}

export function OrbBar({ barAppearance: _barAppearance }: OrbBarProps) {
  // Orb-specific state
  const [agentState, setAgentState] = useState<AgentState>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const [statusLabel, setStatusLabel] = useState("Ready");
  const [currentError, setCurrentError] = useState<string | null>(null);
  const [hasError, setHasError] = useState(false);

  const { resizeWindowIfChanged } = useWindowSize("floating-bar");

  // Dynamic colors via ref — avoids re-rendering the Canvas on color changes
  const colorsRef = useRef<[string, string]>(STATE_COLORS.idle);

  useEffect(() => {
    colorsRef.current = getColorsForOrbState(agentState, hasError);
  }, [agentState, hasError]);

  // Resize window to fit the orb
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
              setAgentState(mapToOrbState(payload.barState));
              setAudioLevel(payload.audioLevel);
              setStatusLabel(getStatusLabel(payload.barState));
              setCurrentError(payload.currentError);
              setHasError(payload.barState === UI.BAR_STATES_ERROR);
            }
          }
        );
        if (mounted) unlisten = fn;
        else fn();
      } catch (error) {
        console.error("OrbBar: Failed to setup event listener:", error);
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
        console.error("OrbBar: Interaction failed:", error);
      });
    },
    []
  );

  const handleClick = useCallback(() => {
    sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sendInteraction]);

  // Feed audioLevel to the right channel based on state:
  // - Listening/input states → manualInput (mic reactivity)
  // - Talking/responding states → manualOutput (speech animation)
  const isTalking = agentState === "talking";
  const manualInput = isTalking ? 0 : audioLevel;
  const manualOutput = isTalking ? audioLevel : 0;

  // Initial colors for the Canvas (colorsRef handles dynamic updates)
  const initialColors = useMemo<[string, string]>(
    () => STATE_COLORS.idle,
    []
  );

  const dragHandlers = useDragWindowWithThreshold();

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full select-none cursor-grab active:cursor-grabbing"
      onClick={handleClick}
      {...dragHandlers}
    >
      <Orb
        agentState={agentState}
        volumeMode="manual"
        manualInput={manualInput}
        manualOutput={manualOutput}
        colors={initialColors}
        colorsRef={colorsRef}
        seed={42}
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
