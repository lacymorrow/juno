"use client";

import type React from "react";
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import {
  Mic,
  Volume2,
  AlertCircle,
  CheckCircle,
  Send,
  X,
  Keyboard,
  Code,
  Video,
  ImageIcon,
  FileText,
  ChevronDown,
  ChevronUp,
  Copy,
  Brain,
  Loader2,
  Check,
  Type,
} from "lucide-react";
import Marquee from "react-fast-marquee";
import AudioVisualizer, { type AppState } from "./audio-visualizer";
import { EVENTS, UI } from "@/lib/constants.generated";
import type {
  VoiceAIBarProps,
  ContentType,
  ResponseContent,
} from "../../types/voice-ai";
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

// === COMPONENT CONSTANTS ===

const FLOATING_BAR_DIMENSIONS = {
  DEFAULT_WIDTH: 120,
  DEFAULT_HEIGHT: 40,
  EXPANDED_WIDTH: 280,
  EXPANDED_HEIGHT: 50,
};

/**
 * Component name for backend interactions - MUST match backend element handling
 */
const COMPONENT_ID = "voice-ai-bar-dark";

/**
 * Maps UI state to AudioVisualizer AppState
 */
const mapUIStateToAppState = (uiState: UIState): AppState => {
  switch (uiState) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
      return UI.AGENT_STATUS_IDLE;
    case UI.BAR_STATES_LISTENING:
      return UI.AGENT_STATUS_LISTENING;
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
      return UI.AGENT_STATUS_PROCESSING;
    case UI.BAR_STATES_SPEAKING:
      return UI.AGENT_STATUS_SPEAKING;
    case UI.BAR_STATES_DICTATING:
      return UI.AGENT_STATUS_DICTATING;
    case UI.BAR_STATES_AGENT_RESPONDING:
      return UI.AGENT_STATUS_RESPONDING;
    case UI.BAR_STATES_ERROR:
      return UI.AGENT_STATUS_ERROR;
    case UI.BAR_STATES_SUCCESS:
      return UI.AGENT_STATUS_SUCCESS;
    case UI.BAR_STATES_INPUT:
      return UI.AGENT_STATUS_INPUT;
    default:
      return UI.AGENT_STATUS_IDLE;
  }
};

export function VoiceAIBar({
  className = "",
  sampleResponses: propSampleResponses,
}: VoiceAIBarProps) {
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

  // Legacy state for visual transitions
  const [localInputValue, setLocalInputValue] = useState("");
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [showStateIcon, setShowStateIcon] = useState(false);
  const [responseContent, setResponseContent] = useState<ResponseContent[]>([]);
  const [isExpanded, setIsExpanded] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const [responsePhase, setResponsePhase] = useState<
    "collapsed" | "expanding-width" | "expanding-height" | "showing-content"
  >("collapsed");
  const [isIdleHovered, setIsIdleHovered] = useState(false);
  const [marqueeKey, setMarqueeKey] = useState(0);
  const [textTransitioning, setTextTransitioning] = useState(false);

  const [contentDimensions, setContentDimensions] = useState({
    width: 0,
    height: 0,
    collapsedHeight: 40,
    summaryHeight: 60,
  });
  const [heightTransitionTarget, setHeightTransitionTarget] = useState<
    "collapsed" | "summary" | "expanded"
  >("collapsed");
  const [isCalculatingDimensions, setIsCalculatingDimensions] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

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
            console.log(
              "📨 VoiceAIBarDark: Received state update:",
              event.payload
            );

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
                "❌ VoiceAIBarDark: Invalid state data received:",
                payload
              );
            }
          }
        );

        console.log("✅ VoiceAIBarDark: Event listener established");
      } catch (error) {
        console.error(
          "❌ VoiceAIBarDark: Failed to setup event listener:",
          error
        );
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        console.log("🔄 VoiceAIBarDark: Event listener cleaned up");
      }
    };
  }, []);

  // === WINDOW RESIZING LOGIC ===

  /**
   * Responsive window resizing based on UI state
   */
  useEffect(() => {
    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const currentUiState = barState.barState;

        // Define compact states that use small window size
        const isCompact = [
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_DICTATION_READY,
        ].includes(currentUiState as any);
        const currentWidth = isCompact ? defaultWidth : EXPANDED_WIDTH;
        const currentHeight = isCompact ? defaultHeight : EXPANDED_HEIGHT;

        console.log(
          `🔧 VoiceAIBarDark: Resizing window to ${currentWidth}x${currentHeight} for state: ${currentUiState}`
        );

        await appWindow.setSize(new LogicalSize(currentWidth, currentHeight));
      } catch (error) {
        console.error("❌ VoiceAIBarDark: Failed to resize window:", error);
      }
    };

    resizeWindow();
  }, [barState.barState]);

  // Default sample responses
  const defaultSampleResponses = {
    text: {
      type: "text" as ContentType,
      title: "About Glass Morphism",
      content:
        "Glass morphism is a design trend characterized by light/dark transparency, vivid colors, floating elements, and subtle borders. It creates a frosted glass effect that gives depth to UI elements while maintaining a modern, clean aesthetic. This style is popular in modern interfaces like macOS Big Sur, iOS, and Windows 11.",
    },
    code: {
      type: "code" as ContentType,
      title: "Glass Effect CSS",
      content: `.glass-effect {
background: rgba(255, 255, 255, 0.15);
backdrop-filter: blur(20px) saturate(180%);
border: 1px solid rgba(255, 255, 255, 0.2);
border-radius: 12px;
padding: 20px;
box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
}`,
    },
    component: {
      type: "component" as ContentType,
      title: "React Glass Button",
      content: `function GlassButton({ children }) {
return (
  <button className="glass-btn">
    {children}
  </button>
);
}

// CSS for the button
const styles = \`
.glass-btn {
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(15px);
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 50px;
  padding: 10px 25px;
  color: white;
  font-weight: 500;
  transition: all 0.3s ease;
}

.glass-btn:hover {
  background: rgba(255, 255, 255, 0.25);
  transform: translateY(-2px);
}
\`;`,
    },
    image: {
      type: "image" as ContentType,
      title: "Glass Morphism Example",
      content:
        "https://images.unsplash.com/photo-1634017839464-5c339ebe3cb4?q=80&w=3000&auto=format&fit=crop",
    },
  };

  // Use provided sample responses or fall back to defaults
  const sampleResponses = propSampleResponses || defaultSampleResponses;

  // === STANDARDIZED INTERACTION HANDLERS ===

  /**
   * Creates a standardized UI interaction event
   * This helper ensures all interactions follow the same pattern
   */
  const createInteraction = useCallback(
    (
      interactionType: string,
      data?: Record<string, any>
    ): UIInteractionEvent => ({
      element_id: COMPONENT_ID,
      interaction_type: interactionType,
      data: data || null,
      timestamp: Date.now(),
    }),
    []
  );

  /**
   * Sends interaction to backend via ui_handle_interaction command
   * This is the standardized way to trigger backend actions
   */
  const sendInteraction = useCallback(
    async (interaction: UIInteractionEvent) => {
      try {
        console.log("🔧 VoiceAIBarDark: Sending interaction:", interaction);

        await invoke("ui_handle_interaction", {
          elementId: COMPONENT_ID,
          interaction,
        });

        console.log("✅ VoiceAIBarDark: Interaction sent successfully");
      } catch (error) {
        console.error("❌ VoiceAIBarDark: Interaction failed:", error);
      }
    },
    []
  );

  /**
   * Sync local input state with backend state updates
   */
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  // Messages for different states (mapped from UI states)
  const getStateMessage = (uiState: UIState) => {
    switch (uiState) {
      case UI.BAR_STATES_DEFAULT:
        return "Ready";
      case UI.BAR_STATES_LISTENING:
        return "Listening to your request...";
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return "Processing your request, please wait while I analyze your input...";
      case UI.BAR_STATES_SPEAKING:
        return "Here's what I found for you based on your request and current context...";
      case UI.BAR_STATES_ERROR:
        return (
          barState.currentError ||
          "Sorry, I couldn't understand that request. Please try speaking more clearly."
        );
      case UI.BAR_STATES_SUCCESS:
        return "Task completed successfully! Is there anything else I can help you with today?";
      case UI.BAR_STATES_INPUT:
        return "Type your request...";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return "Here's what I found:";
      case UI.BAR_STATES_TRANSCRIBING:
        return "Converting speech...";
      case UI.BAR_STATES_DICTATING:
        return "Dictating text...";
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return "Always listening...";
      default:
        return "Processing...";
    }
  };

  // Current message derived from state
  const currentMessage = getStateMessage(barState.barState);

  // === UI STATE CALCULATIONS ===

  const currentUiState = barState.barState;

  // Handle state changes and marquee reset
  useEffect(() => {
    // Handle text transitions
    setTextTransitioning(true);
    setTimeout(() => {
      setTextTransitioning(false);
    }, 150);

    // Reset marquee when state changes
    setMarqueeKey((prev) => prev + 1);

    // Handle specific state transitions
    if (currentUiState === UI.BAR_STATES_INPUT) {
      setTimeout(() => {
        setIsTransitioning(true);
        setTimeout(() => {
          inputRef.current?.focus();
          setIsTransitioning(false);
        }, 300);
      }, 300);
    } else if (currentUiState === UI.BAR_STATES_AGENT_RESPONDING) {
      setTimeout(() => {
        setIsTransitioning(true);
        handleResponseState();
        setTimeout(() => {
          setIsTransitioning(false);
        }, 1200);
      }, 300);
    }

    // Show state icon for success/error states
    if (
      currentUiState === UI.BAR_STATES_SUCCESS ||
      currentUiState === UI.BAR_STATES_ERROR
    ) {
      setTimeout(() => {
        setShowStateIcon(true);
        setTimeout(() => {
          setShowStateIcon(false);
        }, 1500);
      }, 400);
    } else {
      setShowStateIcon(false);
    }
  }, [currentUiState]);

  const handleResponseState = () => {
    setIsCalculatingDimensions(true);
    setResponsePhase("collapsed");

    // Calculate content dimensions first
    setTimeout(() => {
      if (contentRef.current) {
        const rect = contentRef.current.getBoundingClientRect();
        const scrollbarWidth =
          contentRef.current.offsetWidth - contentRef.current.clientWidth;
        const scrollbarHeight =
          contentRef.current.offsetHeight - contentRef.current.clientHeight;

        const collapsedHeight = 40;
        const expandedHeight = Math.min(
          500,
          Math.max(120, rect.height + 100 + scrollbarHeight)
        );

        setContentDimensions({
          width: Math.min(450, Math.max(320, rect.width + 40 + scrollbarWidth)),
          height: expandedHeight,
          collapsedHeight: collapsedHeight,
          summaryHeight: 60,
        });
      }
      setIsCalculatingDimensions(false);

      // Smoother transition timing
      setTimeout(() => {
        setResponsePhase("expanding-width");

        setTimeout(() => {
          setResponsePhase("expanding-height");

          setTimeout(() => {
            setResponsePhase("showing-content");

            setTimeout(() => {
              setIsExpanded(false);
            }, 100);
          }, 600);
        }, 500);
      }, 150);
    }, 100);
  };

  // Handle response state transitions and set sample content
  useEffect(() => {
    if (currentUiState === UI.BAR_STATES_AGENT_RESPONDING) {
      // Set sample response content when entering response state
      setResponseContent([
        sampleResponses.text,
        sampleResponses.code,
        sampleResponses.component,
      ]);
    }
  }, [currentUiState]);

  // === INTERACTION HANDLERS ===

  const toggleListening = useCallback(async () => {
    const interaction = createInteraction("toggle_listening");
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const handleInputSubmit = useCallback(
    async (e?: React.FormEvent) => {
      if (e) e.preventDefault();

      const trimmedValue = localInputValue.trim();
      if (trimmedValue) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_SUBMIT, {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      }
    },
    [localInputValue, createInteraction, sendInteraction]
  );

  const toggleInputMode = useCallback(async () => {
    const interaction = createInteraction("toggle_input");
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const handleFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS);
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const handleBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR);
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const toggleExpanded = useCallback(() => {
    if (!isExpanded) {
      setHeightTransitionTarget("expanded");
      setTimeout(() => {
        setIsExpanded(true);
      }, 300);
    } else {
      setIsExpanded(false);
      setTimeout(() => {
        setHeightTransitionTarget("summary");
      }, 200);
    }
  }, [isExpanded]);

  const closeResponse = useCallback(async () => {
    setIsExpanded(false);
    setResponsePhase("collapsed");
    const interaction = createInteraction("close_response");
    await sendInteraction(interaction);
  }, [createInteraction, sendInteraction]);

  const copyToClipboard = (text: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => {
        console.log("Copied to clipboard!");
      })
      .catch((err) => {
        console.error("Failed to copy: ", err);
      });
  };

  // Get icon based on current state
  const getStateIcon = () => {
    switch (currentUiState) {
      case UI.BAR_STATES_LISTENING:
        return (
          <Mic className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return (
          <Loader2 className="w-3 h-3 text-white transition-all duration-300 animate-spin" />
        );
      case UI.BAR_STATES_SPEAKING:
        return (
          <Volume2 className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_ERROR:
        return (
          <AlertCircle className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_SUCCESS:
        return (
          <Check className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_INPUT:
        return <X className="w-3 h-3 text-white transition-all duration-300" />;
      case UI.BAR_STATES_AGENT_RESPONDING:
        return isExpanded ? (
          <ChevronDown className="w-3 h-3 text-white transition-all duration-300" />
        ) : (
          <ChevronUp className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_TRANSCRIBING:
        return (
          <Mic className="w-3 h-3 text-white transition-all duration-300 animate-pulse" />
        );
      case UI.BAR_STATES_DICTATING:
        return (
          <Type className="w-3 h-3 text-white transition-all duration-300" />
        );
      case UI.BAR_STATES_DICTATION_READY:
        return (
          <Type className="w-3 h-3 text-orange-400 transition-all duration-300" />
        );
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return (
          <Mic className="w-3 h-3 text-blue-400 transition-all duration-300 animate-pulse" />
        );
      default:
        return (
          <Brain className="w-3 h-3 text-white/70 transition-all duration-300" />
        );
    }
  };

  // Get the state feedback icon for waveform replacement
  const getStateFeedbackIcon = () => {
    switch (currentUiState) {
      case UI.BAR_STATES_SUCCESS:
        return (
          <CheckCircle className="w-4 h-4 text-green-400 animate-bounce" />
        );
      case UI.BAR_STATES_ERROR:
        return <AlertCircle className="w-4 h-4 text-red-400 animate-pulse" />;
      default:
        return null;
    }
  };

  // Get content type icon
  const getContentTypeIcon = (type: ContentType) => {
    switch (type) {
      case "code":
        return <Code className="w-4 h-4" />;
      case "component":
        return <Code className="w-4 h-4" />;
      case "image":
        return <ImageIcon className="w-4 h-4" />;
      case "video":
        return <Video className="w-4 h-4" />;
      default:
        return <FileText className="w-4 h-4" />;
    }
  };

  // Get bar class based on current state
  const getBarClass = () => {
    let baseClass =
      currentUiState === UI.BAR_STATES_DEFAULT
        ? "glass-bar-idle"
        : "glass-bar-active";

    if (currentUiState === UI.BAR_STATES_INPUT) {
      baseClass = "glass-bar-input";
    }

    if (currentUiState === UI.BAR_STATES_AGENT_RESPONDING) {
      switch (responsePhase) {
        case "expanding-width":
          baseClass = "glass-bar-response-width";
          break;
        case "expanding-height":
          baseClass = "glass-bar-response-height";
          break;
        case "showing-content":
          if (heightTransitionTarget === "expanded" && isExpanded) {
            baseClass = "glass-bar-response-expanding";
          } else if (heightTransitionTarget === "summary" || !isExpanded) {
            baseClass = "glass-bar-response-summary";
          } else {
            baseClass = "glass-bar-response";
          }
          break;
        default:
          baseClass = "glass-bar-response";
      }
    }

    if (isTransitioning) baseClass += " transitioning";

    // Add state-specific classes
    switch (currentUiState) {
      case UI.BAR_STATES_LISTENING:
        return baseClass + " state-listening";
      case UI.BAR_STATES_LOADING:
      case UI.BAR_STATES_SUBMITTING:
        return baseClass + " state-processing";
      case UI.BAR_STATES_SPEAKING:
        return baseClass + " state-speaking";
      case UI.BAR_STATES_ERROR:
        return baseClass + " state-error";
      case UI.BAR_STATES_SUCCESS:
        return baseClass + " state-success";
      case UI.BAR_STATES_INPUT:
        return baseClass + " state-input";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return baseClass + " state-response";
      default:
        return baseClass;
    }
  };

  // Render content based on type
  const renderContent = (item: ResponseContent) => {
    switch (item.type) {
      case "code":
      case "component":
        return (
          <div className="code-block">
            <div className="code-header">
              <div className="flex items-center gap-1">
                {getContentTypeIcon(item.type)}
                <span>{item.title || "Code"}</span>
              </div>
              <button
                className="copy-btn"
                onClick={() => copyToClipboard(item.content)}
                aria-label="Copy code"
              >
                <Copy className="w-3.5 h-3.5" />
              </button>
            </div>
            <pre className="code-content">
              <code>{item.content}</code>
            </pre>
          </div>
        );
      case "image":
        return (
          <div className="image-block">
            <div className="image-header">
              {getContentTypeIcon(item.type)}
              <span>{item.title || "Image"}</span>
            </div>
            <div className="image-container">
              <img
                src={item.content || "/placeholder.svg"}
                alt={item.title || "AI generated image"}
              />
            </div>
          </div>
        );
      case "video":
        return (
          <div className="video-block">
            <div className="video-header">
              {getContentTypeIcon(item.type)}
              <span>{item.title || "Video"}</span>
            </div>
            <div className="video-container">
              <video controls src={item.content} />
            </div>
          </div>
        );
      default:
        return (
          <div className="text-block">
            <div className="text-header">
              {getContentTypeIcon(item.type)}
              <span>{item.title || "Text"}</span>
            </div>
            <div className="text-content">{item.content}</div>
          </div>
        );
    }
  };

  return (
    <div
      className={`voice-ai-bar-container ${className}`}
      style={
        {
          "--response-width": `${contentDimensions.width}px`,
          "--response-height": `${contentDimensions.height}px`,
          "--summary-height": `${contentDimensions.summaryHeight || 60}px`,
          "--collapsed-height": `${contentDimensions.collapsedHeight || 40}px`,
        } as React.CSSProperties
      }
    >
      {/* Floating Voice Control Bar */}
      <div className={getBarClass()}>
        {/* Text Input Field - Only visible in input state */}
        {currentUiState === UI.BAR_STATES_INPUT && (
          <form onSubmit={handleInputSubmit} className="input-form">
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => setLocalInputValue(e.target.value)}
              onFocus={handleFocus}
              onBlur={handleBlur}
              placeholder="Type your request..."
              className="glass-input"
              autoFocus
            />
            <button
              type="submit"
              className="glass-send-btn"
              disabled={!localInputValue.trim()}
            >
              <Send className="w-3 h-3 text-white" />
            </button>
          </form>
        )}

        {/* Hidden content for dimension calculation */}
        {currentUiState === UI.BAR_STATES_AGENT_RESPONDING &&
          isCalculatingDimensions && (
            <div
              ref={contentRef}
              className="response-content-calculator"
              style={{
                position: "absolute",
                visibility: "hidden",
                pointerEvents: "none",
                width: "350px",
              }}
            >
              <div className="response-header">
                <h3>AI Response</h3>
              </div>
              <div className="response-content">
                {responseContent.map((item, index) => (
                  <div key={index} className="response-item">
                    {renderContent(item)}
                  </div>
                ))}
              </div>
            </div>
          )}

        {/* Response Content - Only visible in response state and after height transition */}
        {currentUiState === UI.BAR_STATES_AGENT_RESPONDING &&
          responsePhase === "showing-content" &&
          !isCalculatingDimensions && (
            <div className="response-container">
              {/* Compact view when collapsed */}
              {!isExpanded && (
                <div className="response-summary" onClick={toggleExpanded}>
                  <div
                    className="response-icon animate-fade-in-delayed"
                    style={{ animationDelay: "200ms" }}
                  >
                    {responseContent.length > 0 &&
                      getContentTypeIcon(responseContent[0].type)}
                  </div>
                  <div
                    className="response-preview animate-fade-in-delayed"
                    style={{ animationDelay: "300ms" }}
                  >
                    {responseContent.length > 0 && (
                      <span className="response-title">
                        {responseContent[0].title || "AI Response"}
                      </span>
                    )}
                  </div>
                </div>
              )}

              {/* Expanded view with full content */}
              {isExpanded && (
                <div
                  className={`response-expanded ${
                    isExpanded ? "expanding" : "collapsing"
                  }`}
                >
                  <div
                    className="response-header animate-fade-in-delayed"
                    style={{ animationDelay: "100ms" }}
                  >
                    <h3>AI Response</h3>
                    <button
                      onClick={closeResponse}
                      className="close-response-btn"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <div className="response-content">
                    {responseContent.map((item, index) => (
                      <div
                        key={index}
                        className="response-item animate-fade-in-up-delayed"
                        style={{ animationDelay: `${300 + index * 250}ms` }}
                      >
                        {renderContent(item)}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

        {/* Audio Visualizer - Replaces the old waveform animation */}
        {/* Audio Visualizer + Status Text - Show both together */}
        {currentUiState !== UI.BAR_STATES_INPUT &&
          currentUiState !== UI.BAR_STATES_AGENT_RESPONDING &&
          currentUiState !== UI.BAR_STATES_DEFAULT &&
          currentUiState !== UI.BAR_STATES_ERROR &&
          currentUiState !== UI.BAR_STATES_SUCCESS && (
            <div className="visualizer-status-container">
              {/* Audio Visualizer */}
              <div className="audio-visualizer-wrapper">
                <div
                  className={`state-feedback ${
                    showStateIcon ? "visible" : "hidden"
                  }`}
                >
                  {getStateFeedbackIcon()}
                </div>
                <div
                  className={`audio-visualizer-content ${
                    showStateIcon ? "hidden" : "visible"
                  }`}
                >
                  <AudioVisualizer
                    appState={mapUIStateToAppState(currentUiState)}
                    width={60}
                    height={20}
                    enableMicrophone={false}
                    intensity={0.8}
                    showTransitionProgress={false}
                    animationStyle="organic"
                    className="audio-visualizer"
                  />
                </div>
              </div>

              {/* Status Text */}
              <div className="status-text-wrapper">
                <div className="status-content">
                  <Marquee
                    key={marqueeKey}
                    speed={25}
                    gradient={true}
                    gradientColor="rgba(255, 255, 255, 0)"
                    gradientWidth={12}
                    pauseOnHover={true}
                    delay={textTransitioning ? 0 : 1.2}
                    play={
                      ![
                        UI.BAR_STATES_DEFAULT,
                        UI.BAR_STATES_ERROR,
                        UI.BAR_STATES_SUCCESS,
                      ].includes(currentUiState as any) &&
                      !isTransitioning &&
                      !textTransitioning
                    }
                  >
                    <span
                      className={`marquee-text text-white/80 text-xs whitespace-nowrap pr-12 ${
                        textTransitioning ? "text-transitioning" : ""
                      }`}
                    >
                      {currentMessage || "Processing..."}
                    </span>
                  </Marquee>
                </div>
              </div>
            </div>
          )}

        {/* Error and Success States - Show icon with marquee text */}
        {(currentUiState === UI.BAR_STATES_ERROR ||
          currentUiState === UI.BAR_STATES_SUCCESS) && (
          <div className="visualizer-status-container">
            {/* State Icon */}
            <div className="audio-visualizer-wrapper">
              <div className="state-feedback visible">
                {getStateFeedbackIcon()}
              </div>
            </div>

            {/* Status Text with Marquee */}
            <div className="status-text-wrapper">
              <div className="status-content">
                <Marquee
                  key={marqueeKey}
                  speed={25}
                  gradient={true}
                  gradientColor="rgba(255, 255, 255, 0)"
                  gradientWidth={12}
                  pauseOnHover={true}
                  delay={textTransitioning ? 0 : 1.2}
                  play={
                    [UI.BAR_STATES_ERROR, UI.BAR_STATES_SUCCESS].includes(
                      currentUiState as any
                    ) &&
                    !isTransitioning &&
                    !textTransitioning
                  }
                >
                  <span
                    className={`marquee-text text-white/80 text-xs whitespace-nowrap pr-12 ${
                      textTransitioning ? "text-transitioning" : ""
                    }`}
                  >
                    {currentMessage || "Processing..."}
                  </span>
                </Marquee>
              </div>
            </div>
          </div>
        )}

        {/* Status Text with Marquee Effect */}
        {/*{assistantState !== "input" &&
        assistantState !== "response" &&
        assistantState !== "idle" &&
        assistantState !== "error" &&
        assistantState !== "success" && (
          <div className={`status-text visible`}>
            <div className="status-content">
              <Marquee
                speed={30}
                gradient={true}
                gradientColor="rgba(255, 255, 255, 0)"
                gradientWidth={8}
                pauseOnHover={true}
                play={assistantState !== "idle" && !isTransitioning}
              >
                <span className="marquee-text text-white/80 text-xs whitespace-nowrap pr-12">
                  {currentMessage || "Processing..."}
                </span>
              </Marquee>
            </div>
          </div>
        )}*/}

        {/* Control Buttons - Idle State with Hover */}
        {currentUiState === UI.BAR_STATES_DEFAULT && (
          <div
            className="idle-container"
            onMouseEnter={() => setIsIdleHovered(true)}
            onMouseLeave={() => setIsIdleHovered(false)}
          >
            <div
              className={`idle-waveform ${
                !isIdleHovered ? "visible" : "hidden"
              }`}
            >
              <AudioVisualizer
                appState={mapUIStateToAppState(UI.BAR_STATES_DEFAULT)}
                width={80}
                height={20}
                enableMicrophone={false}
                intensity={0.6}
                showTransitionProgress={false}
                animationStyle="minimal"
                className="idle-audio-visualizer"
              />
            </div>

            <div
              className={`idle-buttons ${isIdleHovered ? "visible" : "hidden"}`}
            >
              <button
                onClick={toggleListening}
                className="glass-mic-btn"
                disabled={isTransitioning}
                aria-label="Start voice assistant"
              >
                <div className="icon-container">{getStateIcon()}</div>
              </button>

              <button
                onClick={toggleInputMode}
                className="glass-keyboard-btn"
                disabled={isTransitioning}
                aria-label="Use text input"
              >
                <Keyboard className="w-3 h-3 text-white/70" />
              </button>
            </div>
          </div>
        )}

        {currentUiState !== UI.BAR_STATES_DEFAULT &&
          currentUiState !== UI.BAR_STATES_INPUT &&
          currentUiState !== UI.BAR_STATES_AGENT_RESPONDING && (
            <div className="main-button-container">
              <button
                onClick={toggleListening}
                className="glass-mic-btn"
                disabled={
                  currentUiState === UI.BAR_STATES_LOADING ||
                  currentUiState === UI.BAR_STATES_SUBMITTING ||
                  isTransitioning
                }
              >
                <div className="icon-container">{getStateIcon()}</div>
              </button>
            </div>
          )}

        {currentUiState === UI.BAR_STATES_INPUT && (
          <div className="main-button-container">
            <button
              onClick={toggleInputMode}
              className="glass-mic-btn close-btn"
              disabled={isTransitioning}
            >
              <div className="icon-container">{getStateIcon()}</div>
            </button>
          </div>
        )}
      </div>

      <style>
        {`
          .voice-ai-bar-container {
            position: relative;
          }

          .idle-container {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 100%;
            height: 100%;
            position: relative;
          }

          .idle-waveform {
            display: flex;
            align-items: center;
            gap: 0.25rem;
            justify-content: center;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            position: absolute;
          }

          .idle-waveform.visible {
            opacity: 1;
            transform: scale(1);
          }

          .idle-waveform.hidden {
            opacity: 0;
            transform: scale(0.8);
          }

          .idle-buttons {
            display: flex;
            gap: 0.5rem;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            position: absolute;
          }

          .idle-buttons.visible {
            opacity: 1;
            transform: scale(1);
          }

          .idle-buttons.hidden {
            opacity: 0;
            transform: scale(0.8);
            pointer-events: none;
          }

          .audio-visualizer-container {
            display: flex;
            align-items: center;
            justify-content: center;
            position: relative;
            width: auto;
            height: 100%;
          }

          .audio-visualizer-content {
            display: flex;
            align-items: center;
            gap: 0.25rem;
            justify-content: center;
          }

          .state-feedback {
            position: absolute;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.3s ease;
          }

          .state-feedback.visible {
            opacity: 1;
            transform: scale(1);
          }

          .state-feedback.hidden {
            opacity: 0;
            transform: scale(0.8);
          }

          .audio-visualizer-content.visible {
            opacity: 1;
            transform: scale(1);
          }

          .audio-visualizer-content.hidden {
            opacity: 0;
            transform: scale(0.8);
          }

          .glass-bar-idle {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.4rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: center;
            justify-content: center;
            width: 120px;
            height: 40px;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            overflow: hidden;
            cursor: pointer;
          }

          .glass-bar-idle:hover {
            background: radial-gradient(
                circle at 20% 80%,
                rgba(50, 50, 50, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(70, 70, 70, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(40, 40, 40, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(35, 35, 35, 0.95) 0%,
                rgba(25, 25, 25, 0.95) 100%
              );
            border-color: rgba(255, 255, 255, 0.25);
            box-shadow: 0 6px 25px rgba(0, 0, 0, 0.5),
              inset 0 3px 15px rgba(255, 255, 255, 0.08),
              inset 0 -3px 8px rgba(0, 0, 0, 0.4),
              inset 3px 0 8px rgba(255, 255, 255, 0.03),
              inset -3px 0 8px rgba(0, 0, 0, 0.3);
          }

          .glass-bar-active {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem 0.75rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: center;
            gap: 0.75rem;
            width: 240px;
            height: 40px;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            overflow: hidden;
          }

          .glass-bar-input {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem 0.75rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: center;
            gap: 0.75rem;
            width: 280px;
            height: 40px;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            overflow: hidden;
          }

          .glass-bar-response {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem 0.75rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: center;
            gap: 0.75rem;
            width: 280px;
            height: 40px;
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            overflow: hidden;
          }

          .glass-bar-response-width {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem 0.75rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: center;
            gap: 0.75rem;
            width: var(--response-width, 280px);
            height: 40px;
            transition: width 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
            overflow: hidden;
            will-change: width;
            transform: translateZ(0);
          }

          .glass-bar-response-height {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: flex-start;
            gap: 0.75rem;
            width: var(--response-width, 320px);
            height: var(--summary-height, 60px);
            transition: height 0.6s cubic-bezier(0.23, 1, 0.32, 1);
            overflow: hidden;
            will-change: height;
            transform: translateZ(0);
          }

          .glass-bar-response-summary {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: flex-start;
            gap: 0.75rem;
            width: var(--response-width, 320px);
            height: var(--summary-height, 60px);
            transition: height 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
            overflow: hidden;
            will-change: height;
            transform: translateZ(0);
          }

          .glass-bar-response-expanding {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: flex-start;
            gap: 0.75rem;
            width: var(--response-width, 320px);
            height: var(--response-height, 120px);
            transition: height 0.5s cubic-bezier(0.25, 0.46, 0.45, 0.94);
            overflow: hidden;
            will-change: height;
            transform: translateZ(0);
          }

          .glass-bar-response-expanded {
            position: relative;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.3) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.15) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.95) 0%,
                rgba(15, 15, 15, 0.95) 100%
              );
            backdrop-filter: blur(20px) saturate(180%);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 1.5rem;
            padding: 0.5rem;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4),
              inset 0 2px 10px rgba(255, 255, 255, 0.05),
              inset 0 -2px 6px rgba(0, 0, 0, 0.3),
              inset 2px 0 6px rgba(255, 255, 255, 0.02),
              inset -2px 0 6px rgba(0, 0, 0, 0.2);
            display: flex;
            align-items: flex-start;
            gap: 0.75rem;
            width: 400px;
            max-width: 90vw;
            height: auto;
            min-height: 80px;
            max-height: 80vh;
            transition: all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
            overflow: hidden;
            will-change: width, height;
            transform: translateZ(0);
          }

          /* Add granite texture overlay pseudo-elements */
          .glass-bar-idle::before,
          .glass-bar-active::before,
          .glass-bar-input::before,
          .glass-bar-response::before,
          .glass-bar-response-width::before,
          .glass-bar-response-height::before,
          .glass-bar-response-summary::before,
          .glass-bar-response-expanding::before,
          .glass-bar-response-expanded::before {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border-radius: 1.5rem;
            background: repeating-linear-gradient(
                45deg,
                transparent,
                transparent 1px,
                rgba(255, 255, 255, 0.01) 1px,
                rgba(255, 255, 255, 0.01) 2px
              ),
              repeating-linear-gradient(
                -45deg,
                transparent,
                transparent 1px,
                rgba(0, 0, 0, 0.02) 1px,
                rgba(0, 0, 0, 0.02) 2px
              );
            opacity: 0.6;
            mix-blend-mode: overlay;
            pointer-events: none;
            z-index: 1;
          }

          /* Enhanced granite texture overlay */
          .glass-bar-idle::after,
          .glass-bar-active::after,
          .glass-bar-input::after,
          .glass-bar-response::after,
          .glass-bar-response-width::after,
          .glass-bar-response-height::after,
          .glass-bar-response-summary::after,
          .glass-bar-response-expanding::after,
          .glass-bar-response-expanded::after {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border-radius: 1.5rem;
            background: radial-gradient(
                circle at 25% 25%,
                rgba(255, 255, 255, 0.03) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 75% 75%,
                rgba(0, 0, 0, 0.04) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 50% 50%,
                rgba(255, 255, 255, 0.01) 0%,
                transparent 25%
              ),
              noise-pattern;
            opacity: 0.8;
            mix-blend-mode: soft-light;
            pointer-events: none;
            z-index: 2;
          }

          .glass-mic-btn {
            position: relative;
            width: 2rem;
            height: 2rem;
            background: radial-gradient(
                circle at 30% 30%,
                rgba(60, 60, 60, 0.4) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(80, 80, 80, 0.2) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(45, 45, 45, 0.9) 0%,
                rgba(35, 35, 35, 0.9) 100%
              );
            backdrop-filter: blur(15px);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4),
              inset 0 1px 5px rgba(255, 255, 255, 0.05),
              inset 0 -1px 3px rgba(0, 0, 0, 0.3),
              inset 1px 0 3px rgba(255, 255, 255, 0.02);
            flex-shrink: 0;
          }

          .glass-keyboard-btn {
            position: relative;
            width: 2rem;
            height: 2rem;
            background: radial-gradient(
                circle at 30% 30%,
                rgba(60, 60, 60, 0.4) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(80, 80, 80, 0.2) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(45, 45, 45, 0.9) 0%,
                rgba(35, 35, 35, 0.9) 100%
              );
            backdrop-filter: blur(15px);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4),
              inset 0 1px 5px rgba(255, 255, 255, 0.05),
              inset 0 -1px 3px rgba(0, 0, 0, 0.3),
              inset 1px 0 3px rgba(255, 255, 255, 0.02);
            flex-shrink: 0;
          }

          .glass-mic-btn:hover,
          .glass-keyboard-btn:hover {
            background: radial-gradient(
                circle at 30% 30%,
                rgba(80, 80, 80, 0.5) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(100, 100, 100, 0.3) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(65, 65, 65, 0.9) 0%,
                rgba(55, 55, 55, 0.9) 100%
              );
            transform: scale(1.1);
            box-shadow: 0 4px 15px rgba(0, 0, 0, 0.5),
              inset 0 2px 8px rgba(255, 255, 255, 0.08),
              inset 0 -2px 4px rgba(0, 0, 0, 0.4),
              inset 2px 0 4px rgba(255, 255, 255, 0.03);
          }

          /* Button texture overlays */
          .glass-mic-btn::before,
          .glass-keyboard-btn::before {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border-radius: 50%;
            background: repeating-conic-gradient(
              from 0deg at 50% 50%,
              transparent 0deg,
              rgba(255, 255, 255, 0.01) 1deg,
              transparent 2deg
            );
            opacity: 0.5;
            mix-blend-mode: overlay;
            pointer-events: none;
          }

          .state-error {
            animation: pulse-border-red 1.5s ease-in-out infinite,
              shake 0.5s ease-in-out;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(60, 40, 40, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(80, 60, 60, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(50, 30, 30, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(35, 25, 25, 0.95) 0%,
                rgba(25, 15, 15, 0.95) 100%
              ) !important;
            border-color: rgba(239, 68, 68, 0.6) !important;
          }

          .state-success {
            animation: pulse-border-green 1.5s ease-in-out infinite,
              bounce 0.6s ease-in-out;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 60, 40, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 80, 60, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 50, 30, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 35, 25, 0.95) 0%,
                rgba(15, 25, 15, 0.95) 100%
              ) !important;
            border-color: rgba(16, 185, 129, 0.6) !important;
          }

          .state-listening {
            border-color: rgba(59, 130, 246, 0.6);
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 50, 80, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 70, 100, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 40, 70, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 30, 40, 0.95) 0%,
                rgba(15, 20, 30, 0.95) 100%
              );
          }

          .state-processing {
            border-color: rgba(168, 85, 247, 0.6);
            background: radial-gradient(
                circle at 20% 80%,
                rgba(60, 40, 80, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(80, 60, 100, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(50, 30, 70, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(35, 25, 40, 0.95) 0%,
                rgba(25, 15, 30, 0.95) 100%
              );
          }

          .state-speaking {
            border-color: rgba(34, 197, 94, 0.6);
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 60, 50, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 80, 70, 0.2) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 40% 40%,
                rgba(30, 50, 40, 0.5) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 35, 30, 0.95) 0%,
                rgba(15, 25, 20, 0.95) 100%
              );
          }

          .status-text {
            display: flex;
            align-items: center;
            flex: 1;
            min-width: 0;
          }

          .status-content {
            width: 100%;
            overflow: hidden;
          }

          .response-content-calculator {
            max-width: 380px;
            z-index: -1;
            overflow-y: auto;
            max-height: 400px;
          }

          .response-content-calculator .response-header {
            padding: 0.5rem 0.75rem;
            font-size: 0.875rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
          }

          .response-content-calculator .response-content {
            padding: 0.75rem;
            gap: 0.75rem;
            display: flex;
            flex-direction: column;
          }

          @keyframes fade-in-delayed {
            from {
              opacity: 0;
              transform: translateY(4px) scale(0.98);
            }
            to {
              opacity: 1;
              transform: translateY(0) scale(1);
            }
          }

          @keyframes fade-in-up-delayed {
            from {
              opacity: 0;
              transform: translateY(12px) scale(0.96);
            }
            to {
              opacity: 1;
              transform: translateY(0) scale(1);
            }
          }

          .animate-fade-in-delayed {
            opacity: 0;
            animation: fade-in-delayed 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)
              both;
            will-change: opacity, transform;
          }

          .animate-fade-in-up-delayed {
            opacity: 0;
            animation: fade-in-up-delayed 0.5s
              cubic-bezier(0.25, 0.46, 0.45, 0.94) both;
            will-change: opacity, transform;
          }

          @keyframes expand-in {
            from {
              opacity: 0;
              transform: scale(0.95);
            }
            to {
              opacity: 1;
              transform: scale(1);
            }
          }

          @keyframes expand-out {
            from {
              opacity: 1;
              transform: scale(1);
            }
            to {
              opacity: 0;
              transform: scale(0.95);
            }
          }

          .response-expanded.expanding {
            animation: expand-in 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94) both;
          }

          .response-expanded.collapsing {
            animation: expand-out 0.2s cubic-bezier(0.4, 0, 0.2, 1) both;
          }

          @keyframes pulse-border-red {
            0%,
            100% {
              border-color: rgba(239, 68, 68, 0.5);
            }
            50% {
              border-color: rgba(239, 68, 68, 0.8);
            }
          }

          @keyframes pulse-border-green {
            0%,
            100% {
              border-color: rgba(16, 185, 129, 0.5);
            }
            50% {
              border-color: rgba(16, 185, 129, 0.8);
            }
          }

          @keyframes shake {
            0%,
            100% {
              transform: translateX(0);
            }
            25% {
              transform: translateX(-2px);
            }
            75% {
              transform: translateX(2px);
            }
          }

          @keyframes bounce {
            0%,
            100% {
              transform: scale(1);
            }
            50% {
              transform: scale(1.02);
            }
          }

          @media (max-width: 640px) {
            .glass-bar-active {
              padding: 0.4rem 0.6rem;
              gap: 0.5rem;
              width: 180px;
            }

            .glass-bar-input,
            .glass-bar-response {
              width: 240px;
            }

            .glass-bar-response-expanded {
              width: 90vw;
            }

            .glass-mic-btn,
            .glass-keyboard-btn {
              width: 1.75rem;
              height: 1.75rem;
            }
          }

          .visualizer-status-container {
            display: flex;
            align-items: center;
            gap: 0.75rem;
            width: 100%;
            height: 100%;
            flex: 1;
            min-width: 0;
            z-index: 10;
            position: relative;
          }

          .audio-visualizer-wrapper {
            display: flex;
            align-items: center;
            justify-content: center;
            position: relative;
            flex-shrink: 0;
            width: 60px;
            height: 100%;
            z-index: 10;
          }

          .status-text-wrapper {
            display: flex;
            align-items: center;
            flex: 1;
            min-width: 0;
            height: 100%;
            z-index: 10;
          }

          .state-message-container {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            width: 100%;
            height: 100%;
            flex: 1;
            min-width: 0;
            padding: 0 0.25rem;
            z-index: 10;
            position: relative;
          }

          .main-button-container {
            display: flex;
            align-items: center;
            flex-shrink: 0;
            margin-left: auto;
            z-index: 10;
            position: relative;
          }

          .state-icon-wrapper {
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
            z-index: 10;
          }

          .state-text-wrapper {
            display: flex;
            align-items: center;
            flex: 1;
            min-width: 0;
            z-index: 10;
          }

          .state-message {
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 100%;
          }

          .audio-visualizer-content.visible {
            opacity: 1;
            transform: scale(1);
            transition: all 0.3s ease;
          }

          .audio-visualizer-content.hidden {
            opacity: 0;
            transform: scale(0.8);
            transition: all 0.3s ease;
          }

          .state-feedback.visible {
            opacity: 1;
            transform: scale(1);
            transition: all 0.3s ease;
          }

          .state-feedback.hidden {
            opacity: 0;
            transform: scale(0.8);
            transition: all 0.3s ease;
          }

          .glass-bar-idle,
          .glass-bar-active,
          .glass-bar-input,
          .glass-bar-response,
          .glass-bar-response-width,
          .glass-bar-response-height,
          .glass-bar-response-summary,
          .glass-bar-response-expanding {
            transition: all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          }

          .glass-bar-idle.transitioning,
          .glass-bar-active.transitioning,
          .glass-bar-input.transitioning {
            transition: all 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          }

          .visualizer-status-container {
            transition: opacity 0.3s ease, transform 0.3s ease;
          }

          .state-message-container {
            transition: opacity 0.3s ease, transform 0.3s ease;
          }

          .audio-visualizer-content,
          .state-feedback {
            transition: all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          }

          .idle-waveform,
          .idle-buttons {
            transition: all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          }

          .text-transitioning {
            opacity: 0;
            transform: translateY(2px);
            transition: opacity 0.15s ease-out, transform 0.15s ease-out;
          }

          .marquee-text {
            transition: opacity 0.15s ease-out, transform 0.15s ease-out;
          }

          .state-message {
            transition: opacity 0.15s ease-out, transform 0.15s ease-out;
          }

          .status-text-wrapper {
            transition: opacity 0.15s ease-out;
          }

          .state-text-wrapper {
            transition: opacity 0.15s ease-out;
          }

          .input-form {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            flex: 1;
            width: 100%;
            z-index: 10;
            position: relative;
          }

          .glass-input {
            flex: 1;
            background: radial-gradient(
                circle at 20% 80%,
                rgba(30, 30, 30, 0.4) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(50, 50, 50, 0.2) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(15, 15, 15, 0.9) 0%,
                rgba(25, 25, 25, 0.9) 100%
              );
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 1rem;
            padding: 0.25rem 0.75rem;
            color: white;
            font-size: 0.875rem;
            outline: none;
            transition: all 0.3s ease;
            box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.3),
              inset 0 -1px 2px rgba(255, 255, 255, 0.05),
              0 1px 3px rgba(0, 0, 0, 0.2);
          }

          .glass-input::before {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border-radius: 1rem;
            background: repeating-linear-gradient(
              45deg,
              transparent,
              transparent 1px,
              rgba(255, 255, 255, 0.005) 1px,
              rgba(255, 255, 255, 0.005) 2px
            );
            opacity: 0.4;
            mix-blend-mode: overlay;
            pointer-events: none;
          }

          .glass-input::placeholder {
            color: rgba(255, 255, 255, 0.4);
          }

          .glass-input:focus {
            background: radial-gradient(
                circle at 20% 80%,
                rgba(40, 40, 40, 0.5) 0%,
                transparent 50%
              ),
              radial-gradient(
                circle at 80% 20%,
                rgba(60, 60, 60, 0.3) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(25, 25, 25, 0.9) 0%,
                rgba(35, 35, 35, 0.9) 100%
              );
            border-color: rgba(124, 58, 237, 0.4);
            box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.4),
              inset 0 -1px 2px rgba(255, 255, 255, 0.08),
              0 0 0 2px rgba(124, 58, 237, 0.2), 0 2px 6px rgba(0, 0, 0, 0.3);
          }

          .glass-send-btn {
            width: 1.5rem;
            height: 1.5rem;
            background: radial-gradient(
                circle at 30% 30%,
                rgba(150, 80, 255, 0.8) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(124, 58, 237, 0.6) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(124, 58, 237, 0.7) 0%,
                rgba(99, 46, 190, 0.8) 100%
              );
            border: 1px solid rgba(147, 51, 234, 0.4);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.3s ease;
            flex-shrink: 0;
            box-shadow: 0 2px 8px rgba(124, 58, 237, 0.3),
              inset 0 1px 3px rgba(255, 255, 255, 0.1),
              inset 0 -1px 2px rgba(0, 0, 0, 0.3);
          }

          .glass-send-btn::before {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border-radius: 50%;
            background: repeating-conic-gradient(
              from 0deg at 50% 50%,
              transparent 0deg,
              rgba(255, 255, 255, 0.02) 1deg,
              transparent 2deg
            );
            opacity: 0.6;
            mix-blend-mode: overlay;
            pointer-events: none;
          }

          .glass-send-btn:hover {
            background: radial-gradient(
                circle at 30% 30%,
                rgba(170, 100, 255, 0.9) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(147, 78, 255, 0.7) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(147, 78, 255, 0.8) 0%,
                rgba(124, 66, 220, 0.9) 100%
              );
            transform: scale(1.1);
            box-shadow: 0 4px 12px rgba(124, 58, 237, 0.4),
              inset 0 2px 4px rgba(255, 255, 255, 0.15),
              inset 0 -2px 3px rgba(0, 0, 0, 0.4);
          }

          .glass-send-btn:disabled {
            background: radial-gradient(
                circle at 30% 30%,
                rgba(80, 40, 120, 0.4) 0%,
                transparent 70%
              ),
              radial-gradient(
                circle at 70% 70%,
                rgba(60, 30, 90, 0.3) 0%,
                transparent 50%
              ),
              linear-gradient(
                135deg,
                rgba(60, 30, 90, 0.4) 0%,
                rgba(50, 25, 75, 0.5) 100%
              );
            border-color: rgba(80, 40, 120, 0.2);
            cursor: not-allowed;
            transform: scale(1);
            box-shadow: 0 1px 4px rgba(60, 30, 90, 0.2),
              inset 0 1px 2px rgba(255, 255, 255, 0.05),
              inset 0 -1px 1px rgba(0, 0, 0, 0.2);
          }

          .response-container {
            width: 100%;
            height: 100%;
            display: flex;
            flex-direction: column;
            z-index: 10;
            position: relative;
          }

          .response-summary {
            display: flex;
            align-items: center;
            gap: 0.75rem;
            padding: 0.5rem;
            cursor: pointer;
            transition: all 0.3s ease;
            border-radius: 1rem;
          }

          .response-summary:hover {
            background: rgba(255, 255, 255, 0.05);
          }

          .response-icon {
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
            color: white;
          }

          .response-preview {
            flex: 1;
            min-width: 0;
          }

          .response-title {
            color: white;
            font-size: 0.875rem;
            font-weight: 500;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
          }

          .response-expanded {
            width: 100%;
            height: 100%;
            display: flex;
            flex-direction: column;
            overflow: hidden;
          }

          .response-header {
            display: flex;
            align-items: center;
            justify-content: between;
            padding: 0.75rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            flex-shrink: 0;
          }

          .response-header h3 {
            color: white;
            font-size: 1rem;
            font-weight: 600;
            margin: 0;
            flex: 1;
          }

          .close-response-btn {
            background: rgba(255, 255, 255, 0.1);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 50%;
            width: 1.5rem;
            height: 1.5rem;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.3s ease;
            color: white;
          }

          .close-response-btn:hover {
            background: rgba(255, 255, 255, 0.2);
            transform: scale(1.1);
          }

          .response-content {
            flex: 1;
            overflow-y: auto;
            padding: 0.75rem;
            display: flex;
            flex-direction: column;
            gap: 1rem;
          }

          .response-item {
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 0.75rem;
            overflow: hidden;
          }

          .text-block,
          .code-block,
          .image-block,
          .video-block {
            width: 100%;
          }

          .text-header,
          .code-header,
          .image-header,
          .video-header {
            display: flex;
            align-items: center;
            justify-content: between;
            padding: 0.5rem 0.75rem;
            background: rgba(255, 255, 255, 0.05);
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            color: white;
            font-size: 0.875rem;
            font-weight: 500;
            gap: 0.5rem;
          }

          .text-content {
            padding: 0.75rem;
            color: white;
            font-size: 0.875rem;
            line-height: 1.5;
            white-space: pre-wrap;
          }

          .code-content {
            padding: 0.75rem;
            background: rgba(0, 0, 0, 0.3);
            font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
            font-size: 0.8125rem;
            line-height: 1.4;
            overflow-x: auto;
          }

          .code-content code {
            color: white;
            white-space: pre;
          }

          .copy-btn {
            background: rgba(255, 255, 255, 0.1);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 0.375rem;
            padding: 0.25rem;
            cursor: pointer;
            transition: all 0.3s ease;
            color: white;
            display: flex;
            align-items: center;
            justify-content: center;
          }

          .copy-btn:hover {
            background: rgba(255, 255, 255, 0.2);
            transform: scale(1.05);
          }

          .image-container,
          .video-container {
            padding: 0.75rem;
          }

          .image-container img,
          .video-container video {
            width: 100%;
            height: auto;
            border-radius: 0.5rem;
            max-height: 200px;
            object-fit: cover;
          }
        `}
      </style>
    </div>
  );
}
