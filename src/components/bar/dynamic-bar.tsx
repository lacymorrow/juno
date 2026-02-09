"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Brain, Mic, Volume2, AlertCircle, Check, Loader2, Type, Keyboard } from "lucide-react";
import { cn } from "@/lib/utils";

import {
  DynamicIsland,
  DynamicIslandProvider,
  useDynamicIslandSize,
  type SizePresets,
} from "@/components/ui/dynamic-island";
import { EVENTS, UI } from "@/lib/constants.generated";
import tauriConfig from "../../../src-tauri/tauri.conf.json";
import { useWindowSize } from "@/hooks/useWindowSize";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { BarAppearance } from "@/components/bar/barAppearance";
import { getBarLayoutWindowLabel } from "@/components/bar/barAppearance";

// Debounce utility
function debounce<T extends (...args: any[]) => any>(
  func: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: NodeJS.Timeout;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => func(...args), delay);
  };
}

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
  SHADOW_PADDING: 48,
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
        return <Mic className={cn(
          "w-4 h-4 text-blue-400",
          widget.content.level && widget.content.level > 0.5 && "animate-pulse"
        )} />;
      case "volume":
        return <Volume2 className="w-4 h-4 text-green-400" />;
      case "loader":
        return <Loader2 className="w-4 h-4 text-yellow-400 animate-spin" />;
      case "alert":
        return <AlertCircle className="w-4 h-4 text-red-400" />;
      case "check":
        return <Check className="w-4 h-4 text-green-400" />;
      case "type":
        return <Type className="w-4 h-4 text-orange-400" />;
      case "keyboard":
        return <Keyboard className="w-4 h-4 text-purple-400" />;
      case "brain":
      default:
        return <Brain className="w-4 h-4 text-white" />;
    }
  };

  // Special rendering for different widget types
  if (widget.type === "voice" && widget.content.level !== undefined) {
    return (
      <div className="flex items-center justify-center h-full w-full px-4 py-2">
        <div className="flex items-center gap-3">
          {getWidgetIcon()}
          <div className="flex flex-col">
            <span className="text-white text-sm font-medium">
              {widget.content.status}
            </span>
            <div className="flex items-center gap-1 mt-1">
              {[...Array(5)].map((_, i) => (
                <div
                  key={i}
                  className={cn(
                    "w-1 h-3 rounded-full transition-all duration-100",
                    i < Math.ceil(widget.content.level * 5)
                      ? "bg-blue-400"
                      : "bg-white/20"
                  )}
                />
              ))}
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (widget.id === "always-listening" && widget.content.wakeWords) {
    return (
      <div className="flex flex-col items-center justify-center h-full w-full px-4 py-2">
        <div className="flex items-center gap-2 mb-1">
          {getWidgetIcon()}
          <span className="text-white text-sm font-medium">
            {widget.content.status}
          </span>
        </div>
        <div className="text-xs text-white/60">
          Say "{widget.content.wakeWords.join('" or "')}"
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-center h-full w-full px-4 py-2">
      <div className="flex items-center gap-2">
        {getWidgetIcon()}
        <span className="text-white text-sm font-medium truncate max-w-[200px]">
          {widget.content.status}
        </span>
      </div>
    </div>
  );
};

const AIFloatingChatbot = ({
  barAppearance,
}: {
  barAppearance?: BarAppearance;
}) => {
  const { setSize } = useDynamicIslandSize();
  const windowLabel = getCurrentWindow().label;

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

  const layoutWindowLabel = barAppearance
    ? getBarLayoutWindowLabel(barAppearance)
    : windowLabel;
  const floatingBarConfig = tauriConfig.app.windows.find(
    (w) => w.label === layoutWindowLabel
  );

  const defaultWidth =
    floatingBarConfig?.width || FLOATING_BAR_DIMENSIONS.DEFAULT_WIDTH;
  const defaultHeight =
    floatingBarConfig?.height || FLOATING_BAR_DIMENSIONS.DEFAULT_HEIGHT;

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
      safeCleanupEventListener(unlisten);
      console.log("🔄 DynamicBar: Event listener cleaned up");
    };
  }, []);

  // === UI STATE TO WIDGET MAPPING ===

  /**
   * Map UI states to dynamic island sizes and widget data
   */
  const mapStateToWidget = (uiState: UIState): WidgetData => {
    switch (uiState) {
      case UI.BAR_STATES_LISTENING:
        return {
          ...MOCK_WIDGETS.listening,
          content: { status: "Listening...", icon: "mic", level: barState.audioLevel },
        };
      case UI.BAR_STATES_SPEAKING:
        return {
          ...MOCK_WIDGETS.speaking,
          content: { 
            status: barState.spokenText || "Speaking...", 
            icon: "volume" 
          },
        };
      case UI.BAR_STATES_TRANSCRIBING:
        return {
          id: "transcribing",
          type: "voice",
          category: "voice",
          content: { 
            status: barState.transcriptionText || "Converting speech...", 
            icon: "mic" 
          },
          size: "compactLong",
        };
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return {
          ...MOCK_WIDGETS.processing,
          content: { 
            status: barState.inputValue || "Processing...", 
            icon: "loader" 
          },
        };
      case UI.BAR_STATES_ERROR:
        return {
          ...MOCK_WIDGETS.error,
          content: { 
            status: barState.currentError || "Error occurred", 
            icon: "alert" 
          },
        };
      case UI.BAR_STATES_SUCCESS:
        return {
          ...MOCK_WIDGETS.success,
          content: { 
            status: barState.lastSubmittedValue ? "Task completed" : "Success!", 
            icon: "check" 
          },
        };
      case UI.BAR_STATES_AGENT_RESPONDING:
        return {
          id: "agent",
          type: "agent",
          category: "system",
          content: { 
            status: barState.agentState || "Agent working...", 
            icon: "brain" 
          },
          size: "medium",
        };
      case UI.BAR_STATES_DICTATING:
        return {
          id: "dictating",
          type: "voice",
          category: "voice",
          content: { status: "Dictating...", icon: "type" },
          size: "compact",
        };
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return {
          id: "always-listening",
          type: "voice",
          category: "voice",
          content: { 
            status: "Always listening", 
            icon: "mic",
            wakeWords: ["Hey Juno", "Computer"]
          },
          size: "large",
        };
      case UI.BAR_STATES_INPUT:
        return {
          id: "input",
          type: "input",
          category: "system",
          content: { status: "Type your request", icon: "keyboard" },
          size: "long",
        };
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

  // === DYNAMIC WINDOW RESIZING ===
  // Each bar component manages its own sizing based on content
  // This allows for precise, content-aware sizing that the backend cannot predict

  const { resizeWindowIfChanged } = useWindowSize("floating-bar"); // Always use floating-bar window

  /**
   * Calculate optimal window dimensions based on state and content
   */
  const calculateDimensions = useCallback((state: BarStateData) => {
    let dimensions = { width: defaultWidth, height: defaultHeight };
    
    switch (state.barState) {
      case UI.BAR_STATES_DEFAULT:
        dimensions = { width: 80, height: 30 };
        break;
      
      case UI.BAR_STATES_LISTENING:
      case UI.BAR_STATES_TRANSCRIBING:
        dimensions = { width: 160, height: 40 };
        break;
      
      case UI.BAR_STATES_SPEAKING:
        // Dynamic width based on text length
        const textLen = state.spokenText?.length || 0;
        const width = Math.min(320, Math.max(180, 180 + textLen * 2));
        dimensions = { width, height: 45 };
        break;
      
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        dimensions = { width: 200, height: 50 };
        break;
      
      case UI.BAR_STATES_INPUT:
        dimensions = { width: 400, height: 60 };
        break;
      
      case UI.BAR_STATES_ERROR:
        const errorLen = state.currentError?.length || 0;
        const errorWidth = Math.min(350, Math.max(200, 200 + errorLen * 1.5));
        dimensions = { width: errorWidth, height: 55 };
        break;
      
      case UI.BAR_STATES_SUCCESS:
        dimensions = { width: 180, height: 45 };
        break;
      
      case UI.BAR_STATES_AGENT_RESPONDING:
        dimensions = { width: 280, height: 65 };
        break;
      
      case UI.BAR_STATES_ALWAYS_LISTENING:
        dimensions = { width: 250, height: 80 };
        break;
      
      default:
        dimensions = { width: defaultWidth, height: defaultHeight };
    }
    
    return {
      width: dimensions.width + FLOATING_BAR_DIMENSIONS.SHADOW_PADDING,
      height: dimensions.height + FLOATING_BAR_DIMENSIONS.SHADOW_PADDING
    };
  }, [defaultWidth, defaultHeight]);

  // Debounced resize to avoid flickering
  const debouncedResize = useMemo(
    () => debounce((state: BarStateData) => {
      const dimensions = calculateDimensions(state);
      resizeWindowIfChanged(dimensions);
    }, 100),
    [calculateDimensions, resizeWindowIfChanged]
  );

  // Resize window when state changes
  useEffect(() => {
    debouncedResize(barState);
  }, [barState, debouncedResize]);

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
  }, [createInteraction, sendInteraction]);

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
  }, [barState.barState, createInteraction, sendInteraction]);

  const renderCurrentWidget = () => {
    return <WidgetRenderer widget={currentWidgetData} />;
  };

  // Add transition effect between widget changes
  const [isTransitioning, setIsTransitioning] = useState(false);

  useEffect(() => {
    setIsTransitioning(true);
    const timer = setTimeout(() => setIsTransitioning(false), 300);
    return () => clearTimeout(timer);
  }, [currentWidgetData.id]);

  return (
    <div className="h-full w-full relative p-6">
      <div className="flex items-center justify-center h-full">
        <button
          type="button"
          onClick={handleIslandClick}
          className="cursor-pointer bg-transparent p-0 m-0 border-0"
          aria-label="Activate AI panel"
          aria-controls="ai-chatbot-panel"
        >
          <DynamicIsland 
            id="ai-chatbot-panel"
          >
            <div className={cn(
              "transition-all duration-300",
              isTransitioning && "scale-95 opacity-80"
            )}>
              {renderCurrentWidget()}
            </div>
          </DynamicIsland>
        </button>
      </div>
    </div>
  );
};

export function DynamicBar({ barAppearance }: { barAppearance?: BarAppearance }) {
  return (
    <DynamicIslandProvider initialSize={"default"}>
      <div className="h-full w-full bg-transparent">
        <AIFloatingChatbot barAppearance={barAppearance} />
      </div>
    </DynamicIslandProvider>
  );
}
