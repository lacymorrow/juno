import { useEffect, useState, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useWindowSize } from "@/hooks/useWindowSize";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getBarLayoutWindowLabel, type BarAppearance } from "@/components/bar/barAppearance";
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../../src-tauri/tauri.conf.json";

// Import the UI component
import { VoiceControlBar as VoiceControlBarUI } from "@/components/ui/voice-control-bar";
import type { VoiceButtonState } from "@/components/ui/voice-button";

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

// === CONSTANTS ===

const COMPONENT_ID = "voice-control-bar";

const DIMENSIONS = {
  // Matches the UI component size + some padding for shadows
  WIDTH: 220, // Enough for expanded state text
  HEIGHT: 80, // Height + padding
};

export function VoiceControlBar({ barAppearance }: { barAppearance?: BarAppearance }) {
  // === STATE ===
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

  // === WINDOW MANAGEMENT ===
  const windowLabel = getCurrentWindow().label;
  const layoutWindowLabel = barAppearance
    ? getBarLayoutWindowLabel(barAppearance)
    : windowLabel;
  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === layoutWindowLabel
  );

  const { resizeWindowIfChanged } = useWindowSize(windowLabel);

  useEffect(() => {
    // Resize window to fit the bar
    // This bar style has a relatively fixed size compared to others
    resizeWindowIfChanged({
      width: DIMENSIONS.WIDTH,
      height: DIMENSIONS.HEIGHT,
    });
  }, []);

  // === EVENT LISTENER ===
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            if (event.payload && typeof event.payload === "object" && "barState" in event.payload) {
              setBarState(event.payload);
            }
          }
        );
      } catch (error) {
        console.error("VoiceControlBar: Failed to setup listener", error);
      }
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  // === INTERACTIONS ===
  const sendInteraction = async (type: string, data?: Record<string, any>) => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction: {
          element_id: COMPONENT_ID,
          interaction_type: type,
          data: data || null,
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("VoiceControlBar: Interaction failed", error);
    }
  };

  const handleVoicePress = () => {
    sendInteraction("toggle_listening");
  };

  const handleClose = () => {
    // Maybe hide the window or switch back to floating bar?
    // For now, let's just minimize
    invoke("minimize_window");
  };

  // === STATE MAPPING ===
  const visualState: VoiceButtonState = useMemo(() => {
    const s = barState.barState;
    if (s === UI.BAR_STATES_LISTENING || s === UI.BAR_STATES_ALWAYS_LISTENING) return "recording";
    if (
      s === UI.BAR_STATES_LOADING ||
      s === UI.BAR_STATES_SUBMITTING ||
      s === UI.BAR_STATES_TRANSCRIBING ||
      s === UI.BAR_STATES_DICTATING ||
      barState.isAgentWorking
    )
      return "processing";
    if (s === UI.BAR_STATES_SUCCESS || s === UI.BAR_STATES_AGENT_RESPONDING) return "success";
    if (s === UI.BAR_STATES_ERROR) return "error";
    return "idle";
  }, [barState.barState, barState.isAgentWorking]);

  const statusText = useMemo(() => {
    if (barState.currentError) return barState.currentError;
    
    switch (barState.barState) {
      case UI.BAR_STATES_LISTENING:
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return "Listening...";
      case UI.BAR_STATES_TRANSCRIBING:
        return "Transcribing...";
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return "Thinking...";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return "Answering...";
      case UI.BAR_STATES_SUCCESS:
        return "Done!";
      case UI.BAR_STATES_ERROR:
        return "Error";
      default:
        return "Ask AI...";
    }
  }, [barState.barState, barState.currentError]);

  return (
    <div className="flex items-center justify-center w-full h-full p-4 bg-transparent">
      <VoiceControlBarUI
        state={visualState}
        statusText={statusText}
        onVoicePress={handleVoicePress}
        onClose={handleClose}
      />
    </div>
  );
}

