"use client";

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Brain, Mic, Volume2, AlertCircle, Check, Loader2 } from "lucide-react";

import {
  DynamicIsland,
  DynamicIslandProvider,
  useDynamicIslandSize,
  type SizePresets,
} from "@/components/ui/dynamic-island";
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../../src-tauri/tauri.conf.json";

// === STANDARDIZED UI API TYPES ===

/**
 * UI State enumeration - Uses generated constants from backend
 * These values are emitted by the backend UIManager in BAR_STATE_UPDATE events
 */
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

/**
 * Backend State Data Structure - Matches exactly what backend emits
 * This structure is defined in ui_commands.rs emit_bar_state_update()
 */
interface BarStateData {
  // Core state
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;

  // Voice and transcription
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;

  // Status flags
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;

  // Agent state
  agentState: string | null;
}

/**
 * Standardized UI Interaction Event Structure
 * This matches UIInteractionEvent in ui_commands.rs
 */
interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

/**
 * Widget Data Structure for Dynamic Content
 */
interface WidgetData {
  id: string;
  type: string;
  category: string;
  content: any;
  size: SizePresets;
  loading?: boolean;
  error?: string;
}

// === COMPONENT CONSTANTS ===

const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 60,
  DEFAULT_HEIGHT: 20,
  EXPANDED_WIDTH: 280,
  EXPANDED_HEIGHT: 50,
};

/**
 * Component name for backend interactions - MUST match backend element handling
 */
const COMPONENT_ID = "dynamic-bar";

// Mock widget data until agent-driven widgets are implemented
const MOCK_WIDGETS: Record<string, WidgetData> = {
  idle: {
    id: "idle",
    type: "status",
    category: "voice",
    content: { status: "Ready", icon: "brain" },
    size: "default",
  },
  listening: {
    id: "listening",
    type: "voice",
    category: "voice",
    content: { status: "Listening...", icon: "mic" },
    size: "compact",
  },
  speaking: {
    id: "speaking",
    type: "voice",
    category: "voice",
    content: { status: "Speaking...", icon: "volume" },
    size: "compact",
  },
  processing: {
    id: "processing",
    type: "status",
    category: "system",
    content: { status: "Processing...", icon: "loader" },
    size: "compactLong",
  },
  error: {
    id: "error",
    type: "status",
    category: "system",
    content: { status: "Error occurred", icon: "alert" },
    size: "compact",
  },
  success: {
    id: "success",
    type: "status",
    category: "system",
    content: { status: "Success!", icon: "check" },
    size: "compact",
  },
};

const WidgetRenderer = ({ widget }: { widget: WidgetData }) => {
  if (widget.loading) {
    return (
      <div className="flex items-center justify-center h-full w-full">
        <Loader2 className="animate-spin h-6 w-6 text-blue-400" />
      </div>
    );
  }

  if (widget.error) {
    return (
      <div className="flex items-center justify-center h-full w-full px-4">
        <div className="text-center">
          <div className="text-red-400 text-sm font-medium">Error</div>
          <div className="text-gray-400 text-xs">{widget.error}</div>
        </div>
      </div>
    );
  }

  const getWidgetIcon = () => {
    switch (widget.content.icon) {
      case "mic":
        return <Mic className="w-4 h-4 text-blue-400" />;
      case "volume":
        return <Volume2 className="w-4 h-4 text-green-400" />;
      case "loader":
        return <Loader2 className="w-4 h-4 text-yellow-400 animate-spin" />;
      case "alert":
        return <AlertCircle className="w-4 h-4 text-red-400" />;
      case "check":
        return <Check className="w-4 h-4 text-green-400" />;
      case "brain":
      default:
        return <Brain className="w-4 h-4 text-white" />;
    }
  };

  return (
    <div className="flex items-center justify-center h-full w-full px-4 py-2">
      <div className="flex items-center gap-2">
        {getWidgetIcon()}
        <span className="text-white text-sm font-medium">
          {widget.content.status}
        </span>
      </div>
    </div>
  );
};

const AIFloatingChatbot = () => {
  const { setSize } = useDynamicIslandSize();

  // === STATE MANAGEMENT ===

  /**
   * Backend-driven state - Updated via BAR_STATE_UPDATE events
   * This is the single source of truth for all UI state
   */
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

  const [currentWidgetData, setCurrentWidgetData] = useState<WidgetData>(
    MOCK_WIDGETS.idle
  );

  // === WINDOW CONFIGURATION ===

  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === "floating-bar"
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;
  const EXPANDED_WIDTH = FLOATING_BAR_DIMENSIONS.EXPANDED_WIDTH;
  const EXPANDED_HEIGHT = FLOATING_BAR_DIMENSIONS.EXPANDED_HEIGHT;

  // === STANDARDIZED EVENT LISTENER ===

  /**
   * Primary backend integration: Listen to BAR_STATE_UPDATE events
   * This is the core pattern for all UI components - event-driven state updates
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            console.log("📨 DynamicBar: Received state update:", event.payload);

            // Validate the received data structure
            const payload = event.payload;
            if (
              payload &&
              typeof payload === "object" &&
              "barState" in payload
            ) {
              setBarState(payload);
            } else {
              console.error(
                "❌ DynamicBar: Invalid state data received:",
                payload
              );
            }
          }
        );

        console.log("✅ DynamicBar: Event listener established");
      } catch (error) {
        console.error("❌ DynamicBar: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        console.log("🔄 DynamicBar: Event listener cleaned up");
      }
    };
  }, []);

  // === UI STATE TO WIDGET MAPPING ===

  /**
   * Map UI states to dynamic island sizes and widget data
   */
  const mapStateToWidget = (uiState: UIState): WidgetData => {
    switch (uiState) {
      case UI.BAR_STATES_LISTENING:
        return MOCK_WIDGETS.listening;
      case UI.BAR_STATES_SPEAKING:
        return MOCK_WIDGETS.speaking;
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return MOCK_WIDGETS.processing;
      case UI.BAR_STATES_ERROR:
        return MOCK_WIDGETS.error;
      case UI.BAR_STATES_SUCCESS:
        return MOCK_WIDGETS.success;
      case UI.BAR_STATES_DEFAULT:
      default:
        return MOCK_WIDGETS.idle;
    }
  };

  // Update widget data and size based on backend state
  useEffect(() => {
    const newWidget = mapStateToWidget(barState.barState);
    setCurrentWidgetData(newWidget);
    setSize(newWidget.size);
  }, [barState.barState, setSize]);

  // === DYNAMIC WINDOW RESIZING LOGIC ===

  /**
   * Smart window sizing based on state and content
   */
  const getWindowDimensions = (uiState: UIState, widget: WidgetData) => {
    // Base dimensions from config
    const base = { width: defaultWidth, height: defaultHeight };

    // State-specific sizing with multiple tiers
    switch (uiState) {
      case UI.BAR_STATES_DEFAULT:
      case UI.BAR_STATES_DICTATION_READY:
        return { width: 80, height: 30 }; // Extra compact for default state

      case UI.BAR_STATES_LISTENING:
      case UI.BAR_STATES_TRANSCRIBING:
        return { width: 160, height: 40 }; // Medium for voice states

      case UI.BAR_STATES_SPEAKING:
        // Size based on content length
        const textLength = barState.spokenText?.length || 0;
        const dynamicWidth = Math.min(320, Math.max(180, 180 + textLength * 2));
        return { width: dynamicWidth, height: 45 };

      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return { width: 200, height: 50 }; // Medium for processing states

      case UI.BAR_STATES_INPUT:
      case UI.BAR_STATES_EXPANDING:
        // Large for input state
        return { width: 400, height: 60 };

      case UI.BAR_STATES_ERROR:
        // Size based on error message length
        const errorLength = barState.currentError?.length || 0;
        const errorWidth = Math.min(
          350,
          Math.max(200, 200 + errorLength * 1.5)
        );
        return { width: errorWidth, height: 55 };

      case UI.BAR_STATES_SUCCESS:
        return { width: 180, height: 45 }; // Compact for success

      case UI.BAR_STATES_AGENT_RESPONDING:
        // Dynamic size based on agent state
        return { width: 280, height: 65 }; // Larger for agent work

      case UI.BAR_STATES_ALWAYS_LISTENING:
        // Large persistent state
        return { width: 250, height: 80 };

      default:
        return base;
    }
  };

  /**
   * Enhanced window resizing with smooth transitions and content awareness
   */
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const dimensions = getWindowDimensions(
          barState.barState,
          currentWidgetData
        );

        console.log(
          `🔧 DynamicBar: Smart resizing to ${dimensions.width}x${dimensions.height} for state: ${barState.barState}`
        );

        await appWindow.setSize(
          new LogicalSize(dimensions.width, dimensions.height)
        );
      } catch (error) {
        console.error("❌ DynamicBar: Failed to resize window:", error);
      }
    };

    resizeWindow();
  }, [
    barState.barState,
    currentWidgetData,
    barState.spokenText,
    barState.currentError,
  ]);

  // === STANDARDIZED INTERACTION HANDLERS ===

  /**
   * Creates a standardized UI interaction event
   * This helper ensures all interactions follow the same pattern
   */
  const createInteraction = (
    interactionType: string,
    data?: Record<string, any>
  ): UIInteractionEvent => ({
    element_id: COMPONENT_ID,
    interaction_type: interactionType,
    data: data || null,
    timestamp: Date.now(),
  });

  /**
   * Sends interaction to backend via ui_handle_interaction command
   * This is the standardized way to trigger backend actions
   */
  const sendInteraction = async (interaction: UIInteractionEvent) => {
    try {
      console.log("🔧 DynamicBar: Sending interaction:", interaction);

      await invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction,
      });

      console.log("✅ DynamicBar: Interaction sent successfully");
    } catch (error) {
      console.error("❌ DynamicBar: Interaction failed:", error);
    }
  };

  /**
   * Handle dynamic island click interactions
   */
  const handleIslandClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle keyboard shortcuts (especially Escape key)
   */
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Handle escape key to cancel or close
      if (event.key === "Escape") {
        const interaction = createInteraction(UI.INTERACTION_TYPES_ESCAPE);
        sendInteraction(interaction);
      }

      // Handle Enter key for quick actions
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_ENTER);
        sendInteraction(interaction);
      }
    };

    // Only add keyboard listeners if this component is active
    if (barState.barState !== UI.BAR_STATES_DEFAULT) {
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [barState.barState]);

  const renderCurrentWidget = () => {
    return <WidgetRenderer widget={currentWidgetData} />;
  };

  return (
    <div className="h-full w-full relative">
      <div className="flex items-center justify-center h-full">
        <div onClick={handleIslandClick} className="cursor-pointer">
          <DynamicIsland id="ai-chatbot-panel">
            {renderCurrentWidget()}
          </DynamicIsland>
        </div>
      </div>
    </div>
  );
};

export function DynamicIslandDemo() {
  return (
    <DynamicIslandProvider initialSize={"default"}>
      <div className="h-full w-full">
        <AIFloatingChatbot />
      </div>
    </DynamicIslandProvider>
  );
}
