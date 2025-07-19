"use client";

import type React from "react";
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { safeUnlisten } from "@/lib/tauri-event-utils";
import {
  AlertCircle,
  CheckCircle,
  Send,
  X,
  Code,
  Video,
  ImageIcon,
  FileText,
  Copy,
} from "lucide-react";
import Marquee from "react-fast-marquee";
import AudioVisualizer, { type AppState } from "./audio-visualizer";

import { EVENTS, UI } from "@/lib/constants.generated";
import type {
  VoiceAIBarProps,
  ContentType,
  ResponseContent,
} from "../../types/voice-ai";
// Window dimensions from config
const WINDOW_WIDTH = 400;
const WINDOW_HEIGHT = 600;

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

const defaultSampleResponses = {
  text: {
    type: "text" as ContentType,
    title: "Glass Morphism",
    content:
      "Glass morphism is a design trend characterized by light/dark transparency, vivid colors, floating elements, and subtle borders. It creates a frosted glass effect that gives depth to UI elements while maintaining a modern, clean aesthetic. This style is popular in modern interfaces like macOS Big Sur, iOS, and Windows 11.",
  },
  component: {
    type: "component" as ContentType,
    title: "Design System Components",
    content: "Loading component library...",
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
  reactComponent: {
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

// === BASE COMPONENT PROPS ===

export interface VoiceAIBarBaseProps extends VoiceAIBarProps {
  theme?: 'light' | 'dark';
  componentId?: string;
}

export function VoiceAIBarBase({
  className = "",
  sampleResponses: propSampleResponses,
  theme = 'light',
  componentId,
}: VoiceAIBarBaseProps) {
  // Determine component ID based on theme or custom ID
  const COMPONENT_ID = componentId || (theme === 'dark' ? 'voice-ai-bar-dark' : 'voice-ai-bar');

  // Backend-managed state
  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    voiceMode: "inactive",
    audioLevel: 0,
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    agentState: null,
  });

  // Theme-specific state (only used in dark theme)
  const isTransitioning = theme === 'dark' ? false : undefined;
  const textTransitioning = theme === 'dark' ? false : undefined;
  
  const [responseContent, setResponseContent] = useState<ResponseContent[]>([]);
  const [isExpanded] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const [animationState] = useState<
    "collapsed" | "expanding" | "expanded" | "collapsing"
  >("collapsed");
  const [isIdleHovered, setIsIdleHovered] = useState(false);
  const [marqueeKey] = useState(0);

  // const [contentDimensions, setContentDimensions] = useState({
  //   width: 0,
  //   height: 0,
  // });

  const [expandReason, setExpandReason] = useState<"width" | "height" | null>(
    null
  );
  const contentRef = useRef<HTMLDivElement>(null);

  // Debounce window resize commands
  const resizeTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Window resize logic - commented out as it's not currently used
  // const setWindowSize = useCallback(
  //   async (size: LogicalSize) => {
  //     if (resizeTimeoutRef.current) {
  //       clearTimeout(resizeTimeoutRef.current);
  //     }

  //     resizeTimeoutRef.current = setTimeout(async () => {
  //       try {
  //         const currentWindow = getCurrentWindow();
  //         await currentWindow.setSize(size);
  //       } catch (error) {
  //         console.error("Failed to resize window:", error);
  //       }
  //     }, 50);
  //   },
  //   []
  // );

  // Setup event listener for backend state updates
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            console.log(
              `📨 ${COMPONENT_ID}: Received state update:`,
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
                `❌ ${COMPONENT_ID}: Invalid state data received:`,
                payload
              );
            }
          }
        );

        console.log(`✅ ${COMPONENT_ID}: Event listener established`);
      } catch (error) {
        console.error(`❌ ${COMPONENT_ID}: Failed to setup event listener:`, error);
      }
    };

    setupListener();

    return () => {
      safeUnlisten(unlisten);
      if (resizeTimeoutRef.current) {
        clearTimeout(resizeTimeoutRef.current);
      }
    };
  }, [COMPONENT_ID]);

  // === STANDARDIZED INTERACTION HANDLERS ===

  /**
   * Creates a standardized UI interaction event
   * This helper ensures all interactions follow the same pattern
   */
  const createInteractionEvent = (
    elementId: string,
    interactionType: string,
    data?: Record<string, any>
  ): UIInteractionEvent => ({
    element_id: elementId,
    interaction_type: interactionType,
    data: data || null,
    timestamp: Date.now(),
  });

  /**
   * Sends UI interaction event to backend
   * All component interactions should use this method
   */
  const sendInteraction = async (
    elementId: string,
    interactionType: string,
    data?: Record<string, any>
  ) => {
    const event = createInteractionEvent(elementId, interactionType, data);
    console.log(`🔄 ${COMPONENT_ID}: Sending interaction:`, event);

    try {
      await invoke("ui_interaction", { event });
    } catch (error) {
      console.error(`❌ ${COMPONENT_ID}: Failed to send interaction:`, error);
    }
  };

  // === COMPONENT BEHAVIOR ===

  // Use provided sample responses or fall back to defaults
  const sampleResponses = propSampleResponses || defaultSampleResponses;

  // Messages for different states (mapped from UI states)
  const getStateMessage = (uiState: UIState) => {
    switch (uiState) {
      case UI.BAR_STATES_DEFAULT:
        return "Ready";
      case UI.BAR_STATES_EXPANDING:
        return "Expanding...";
      case UI.BAR_STATES_INPUT:
        return "Type your message";
      case UI.BAR_STATES_SHRINKING:
        return "Closing...";
      case UI.BAR_STATES_SUBMITTING:
        return "Sending...";
      case UI.BAR_STATES_LOADING:
        return "Processing...";
      case UI.BAR_STATES_FINISHING:
        return "Finishing...";
      case UI.BAR_STATES_SUCCESS:
        return "Complete!";
      case UI.BAR_STATES_LISTENING:
        return "Listening...";
      case UI.BAR_STATES_ERROR:
        return barState.currentError || "Error occurred";
      case UI.BAR_STATES_TRANSCRIBING:
        return barState.transcriptionText || "Transcribing...";
      case UI.BAR_STATES_SPEAKING:
        return barState.spokenText || "Speaking...";
      case UI.BAR_STATES_DICTATING:
        return "Dictating...";
      case UI.BAR_STATES_DICTATION_READY:
        return "Dictation ready";
      case UI.BAR_STATES_ALWAYS_LISTENING:
        return "Always listening...";
      case UI.BAR_STATES_AGENT_RESPONDING:
        return barState.agentState || "Thinking...";
      default:
        return "Processing...";
    }
  };

  // Computed values based on UI state
  const isInputMode = barState.barState === UI.BAR_STATES_INPUT;
  const isError = barState.barState === UI.BAR_STATES_ERROR;
  const isSuccess = barState.barState === UI.BAR_STATES_SUCCESS;
  // const isProcessing = [
  //   UI.BAR_STATES_SUBMITTING,
  //   UI.BAR_STATES_LOADING,
  //   UI.BAR_STATES_AGENT_RESPONDING,
  // ].includes(barState.barState as any);


  // Hover handlers for idle state
  const handleIdleHover = (hovering: boolean) => {
    setIsIdleHovered(hovering);
    if (hovering) {
      sendInteraction(`${COMPONENT_ID}-idle`, "hover");
    }
  };

  // Click handler for idle state
  const handleIdleClick = () => {
    sendInteraction(`${COMPONENT_ID}-idle`, "click");
  };

  // Toggle between voice and keyboard input modes
  const toggleInputMode = () => {
    sendInteraction(
      `${COMPONENT_ID}-toggle-mode`,
      "click",
      {
        currentMode: barState.isDictationMode ? "dictation" : "keyboard",
      }
    );
  };

  // Handle input submission
  const handleInputSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const value = barState.inputValue.trim();
    if (value) {
      sendInteraction(`${COMPONENT_ID}-input`, "submit", { value });
    }
  };

  // Handle sample response click
  const handleSampleClick = (response: any) => {
    setResponseContent([response]);
    if (contentRef.current) {
      const { scrollWidth, scrollHeight, clientWidth, clientHeight } =
        contentRef.current;

      const needsWidthExpansion = scrollWidth > clientWidth;
      const needsHeightExpansion = scrollHeight > clientHeight + 20;

      if (needsWidthExpansion || needsHeightExpansion) {
        setExpandReason(needsWidthExpansion ? "width" : "height");
        sendInteraction(`${COMPONENT_ID}-sample`, "click", {
          expandReason: needsWidthExpansion ? "width" : "height",
          response,
        });
      } else {
        sendInteraction(`${COMPONENT_ID}-sample`, "click", { response });
      }
    }
  };

  // Copy code content
  const copyCode = (code: string) => {
    navigator.clipboard.writeText(code);
    sendInteraction(`${COMPONENT_ID}-copy`, "click", { code });
  };

  // Dynamic class name based on state
  const getBarClass = () => {
    let baseClass = "glass-bar";

    if (barState.barState === UI.BAR_STATES_DEFAULT) {
      baseClass = isIdleHovered
        ? "glass-bar-idle"
        : "glass-bar-active";
    } else if (isInputMode) {
      baseClass = "glass-bar-input";
    } else if (isExpanded) {
      switch (animationState) {
        case "expanding":
          if (expandReason === "width") {
            baseClass = "glass-bar-response-width";
          } else {
            baseClass = "glass-bar-response-height";
          }
          break;
        case "expanded":
          if (expandReason === "height") {
            baseClass = "glass-bar-response-expanding";
          } else if (responseContent.length > 0) {
            baseClass = "glass-bar-response-summary";
          } else {
            baseClass = "glass-bar-response";
          }
          break;
        case "collapsing":
          baseClass = "glass-bar-response";
          break;
      }
    } else {
      baseClass = "glass-bar-active";
    }

    if (animationState === "expanded") {
      baseClass += " glass-bar-response-expanded";
    }

    return baseClass;
  };

  // Get icon for state feedback
  const getStateFeedbackIcon = () => {
    if (isSuccess) {
      return <CheckCircle className="w-4 h-4 text-green-400 animate-bounce" />;
    }
    if (isError) {
      return <AlertCircle className="w-4 h-4 text-red-400 animate-pulse" />;
    }
    return null;
  };

  // Get content type icon
  const getContentIcon = (type: ContentType) => {
    switch (type) {
      case "component":
        return <Code className="w-4 h-4" />;
      case "code":
        return <Code className="w-4 h-4" />;
      case "image":
        return <ImageIcon className="w-4 h-4" />;
      case "video":
        return <Video className="w-4 h-4" />;
      case "text":
        return <FileText className="w-4 h-4" />;
      default:
        return null;
    }
  };

  // Render response content
  const renderResponseContent = () => {
    if (responseContent.length === 0) return null;

    return (
      <div className="response-content-wrapper animate-fade-in-delayed">
        {responseContent.map((item, index) => (
          <div key={index} className="response-item animate-fade-in-up-delayed">
            {item.type === "code" && (
              <div className="code-block">
                <div className="code-header">
                  <div className="flex items-center gap-1">
                    {getContentIcon(item.type)}
                    <span className="text-xs">{item.title}</span>
                  </div>
                  <button
                    className="copy-btn"
                    onClick={() => copyCode(item.content)}
                    aria-label="Copy code"
                  >
                    <Copy className="w-3.5 h-3.5" />
                  </button>
                </div>
                <pre className="code-content">
                  <code>{item.content}</code>
                </pre>
              </div>
            )}

            {item.type === "image" && (
              <div className="image-block">
                <div className="image-header">
                  {getContentIcon(item.type)}
                  <span className="text-xs">{item.title}</span>
                </div>
                <div className="image-container">
                  <img
                    src={item.content}
                    alt={item.title}
                    className="w-full h-auto"
                  />
                </div>
              </div>
            )}

            {item.type === "video" && (
              <div className="video-block">
                <div className="video-header">
                  {getContentIcon(item.type)}
                  <span className="text-xs">{item.title}</span>
                </div>
                <div className="video-container">
                  <video src={item.content} controls className="w-full h-auto" />
                </div>
              </div>
            )}

            {item.type === "text" && (
              <div className="text-block">
                <div className="text-header">
                  {getContentIcon(item.type)}
                  <span className="text-xs">{item.title}</span>
                </div>
                <div className="text-content">{item.content}</div>
              </div>
            )}
          </div>
        ))}
      </div>
    );
  };

  // Current message based on state
  const currentMessage = getStateMessage(barState.barState as UIState);
  // const assistantState = getAssistantStateDisplay();

  // Determine marquee gradient color based on theme
  const marqueeGradientColor = theme === 'dark' ? "rgba(255, 255, 255, 0)" : "rgba(255, 255, 255, 0)";
  
  // Get mapped app state for audio visualizer
  const appState = mapUIStateToAppState(barState.barState as UIState);
  
  // Get icon based on state - commented out as it's not currently used
  /* const getStateIcon = () => {
    const currentUiState = barState.barState;
    
    if (theme === 'dark') {
      // Dark theme state icons
      switch (currentUiState) {
        case UI.BAR_STATES_LISTENING:
        case UI.BAR_STATES_TRANSCRIBING:
          return (
            <Mic className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_LOADING:
        case UI.BAR_STATES_SUBMITTING:
        case UI.BAR_STATES_AGENT_RESPONDING:
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
        case UI.BAR_STATES_DEFAULT:
          return isIdleHovered ? (
            <ChevronDown className="w-3 h-3 text-white transition-all duration-300" />
          ) : (
            <ChevronUp className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_DICTATING:
          return (
            <Mic className="w-3 h-3 text-white transition-all duration-300 animate-pulse" />
          );
        case UI.BAR_STATES_DICTATION_READY:
          return (
            <Type className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_ALWAYS_LISTENING:
          return (
            <Type className="w-3 h-3 text-orange-400 transition-all duration-300" />
          );
        default:
          return (
            <Mic className="w-3 h-3 text-blue-400 transition-all duration-300 animate-pulse" />
          );
      }
    } else {
      // Light theme state icons (original behavior)
      switch (currentUiState) {
        case UI.BAR_STATES_LISTENING:
        case UI.BAR_STATES_TRANSCRIBING:
          return (
            <Mic className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_LOADING:
        case UI.BAR_STATES_SUBMITTING:
        case UI.BAR_STATES_AGENT_RESPONDING:
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
        case UI.BAR_STATES_DEFAULT:
          return isIdleHovered ? (
            <ChevronDown className="w-3 h-3 text-white transition-all duration-300" />
          ) : (
            <ChevronUp className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_DICTATING:
          return (
            <Mic className="w-3 h-3 text-white transition-all duration-300 animate-pulse" />
          );
        case UI.BAR_STATES_DICTATION_READY:
          return (
            <Type className="w-3 h-3 text-white transition-all duration-300" />
          );
        case UI.BAR_STATES_ALWAYS_LISTENING:
          return (
            <Type className="w-3 h-3 text-orange-400 transition-all duration-300" />
          );
        default:
          return (
            <Brain className="w-3 h-3 text-white/70 transition-all duration-300" />
          );
      }
    }
  }; */

  return (
    <div
      className={`voice-ai-bar-container ${className}`}
      style={
        {
          "--window-width": `${WINDOW_WIDTH}px`,
          "--window-height": `${WINDOW_HEIGHT}px`,
          "--expanded-width": "min(600px, 90vw)",
          "--expanded-height": "400px",
        } as React.CSSProperties
      }
    >
      <div className={getBarClass()}>
        {/* Idle State - Glowing orb */}
        {barState.barState === UI.BAR_STATES_DEFAULT && (
          <button
            className={`glass-mic-btn`}
            onClick={handleIdleClick}
            onMouseEnter={() => handleIdleHover(true)}
            onMouseLeave={() => handleIdleHover(false)}
            aria-label="Activate assistant"
          >
            <div className="visualizer-status-container">
              <div className="audio-visualizer-wrapper">
                <AudioVisualizer
                  appState={appState}
                  className="w-full h-full"
                />
              </div>
            </div>
          </button>
        )}

        {/* Input Mode */}
        {isInputMode && (
          <form onSubmit={handleInputSubmit} className="input-form">
            <input
              ref={inputRef}
              type="text"
              value={barState.inputValue}
              onChange={(e) =>
                sendInteraction(`${COMPONENT_ID}-input`, "change", {
                  value: e.target.value,
                })
              }
              className="glass-input"
              placeholder="Type your message..."
              autoFocus
            />
            <button
              type="submit"
              className="glass-send-btn"
              disabled={!barState.inputValue.trim()}
            >
              <Send className="w-3 h-3 text-white" />
            </button>
          </form>
        )}

        {/* Response Content Container */}
        {isExpanded && responseContent.length > 0 && (
          <div
            ref={contentRef}
            className="response-content-calculator"
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              opacity: 0,
              pointerEvents: "none",
              width: "max-content",
              maxWidth: "90vw",
            }}
          >
            {renderResponseContent()}
          </div>
        )}

        {/* Response Content Display */}
        {isExpanded && responseContent.length > 0 && (
          <div className="response-expanded expanding">
            <div className="response-header">
              <h3 className="response-title animate-fade-in-delayed">
                Glass Morphism Guide
              </h3>
              <div className="response-samples animate-fade-in-delayed">
                {Object.entries(sampleResponses).map(([key, response]) => (
                  <button
                    key={key}
                    onClick={() => handleSampleClick(response)}
                    className={`sample-btn`}
                    style={{ animationDelay: "200ms" }}
                  >
                    {getContentIcon(response.type)}
                  </button>
                ))}
                <button
                  onClick={() => sendInteraction(`${COMPONENT_ID}-close`, "click")}
                  className="close-btn"
                  style={{ animationDelay: "300ms" }}
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
            {renderResponseContent()}
          </div>
        )}

        {/* Default Active State */}
        {barState.barState !== UI.BAR_STATES_DEFAULT &&
          !isInputMode &&
          !isExpanded && (
            <>
              {/* Visualizer and Icon */}
              <div className="visualizer-container">
                <button
                  onClick={toggleInputMode}
                  className="glass-mic-btn"
                  aria-label="Toggle input mode"
                  style={{ animationDelay: "100ms" }}
                >
                  <div className="visualizer-status-container">
                    <div className="audio-visualizer-wrapper">
                      <AudioVisualizer
                        appState={appState}
                        className="w-full h-full"
                      />
                    </div>
                  </div>
                </button>

                {sampleResponses && (
                  <div className="sample-buttons">
                    {Object.entries(sampleResponses).map(
                      ([key, response], index) => (
                        <button
                          key={key}
                          onClick={() => handleSampleClick(response)}
                          className={`sample-btn`}
                          style={{ animationDelay: `${300 + index * 250}ms` }}
                          aria-label={`Sample: ${response.title}`}
                        >
                          {getContentIcon(response.type)}
                        </button>
                      )
                    )}
                  </div>
                )}
              </div>

              {/* Status Text - Scrolling marquee for most states */}
              {barState.barState !== UI.BAR_STATES_ERROR &&
                barState.barState !== UI.BAR_STATES_SUCCESS && (
                  <div className="status-text-container">
                    <div className="status-text-wrapper">
                      <div className="status-content">
                        <Marquee
                          key={marqueeKey}
                          speed={theme === 'dark' ? 25 : 30}
                          gradient={true}
                          gradientColor={marqueeGradientColor}
                          gradientWidth={theme === 'dark' ? 12 : 8}
                          pauseOnHover={true}
                          delay={
                            theme === 'dark' ? (textTransitioning ? 0 : 1.2) : 1.5
                          }
                          play={
                            theme === 'dark' 
                              ? ![
                                  UI.BAR_STATES_DEFAULT,
                                  UI.BAR_STATES_ERROR,
                                  UI.BAR_STATES_SUCCESS,
                                ].includes(barState.barState as any) &&
                                !isTransitioning &&
                                !textTransitioning
                              : barState.barState !== (UI.BAR_STATES_DEFAULT as any)
                          }
                        >
                          <span
                            className={`marquee-text text-white/80 text-xs whitespace-nowrap pr-12 ${
                              theme === 'dark' && textTransitioning ? 'text-transitioning' : ''
                            }`}
                          >
                            {currentMessage || "Processing..."}
                          </span>
                        </Marquee>
                      </div>
                    </div>
                  </div>
                )}

              {/* Error and Success States - Show icon with text */}
              {(barState.barState === UI.BAR_STATES_ERROR ||
                barState.barState === UI.BAR_STATES_SUCCESS) && (
                <div className="state-message-container">
                  <div className="state-icon-wrapper">{getStateFeedbackIcon()}</div>
                  <div className="state-text-wrapper">
                    <span className="state-message text-white/90 text-xs font-medium">
                      {currentMessage}
                    </span>
                  </div>
                </div>
              )}

              {/* Close Button */}
              {barState.barState === UI.BAR_STATES_INPUT && (
                <button onClick={toggleInputMode} className="glass-mic-btn close-btn">
                  <X className="w-3 h-3 text-white" />
                </button>
              )}
            </>
          )}
      </div>

      <style dangerouslySetInnerHTML={{ __html: `
        /* CSS Variables for theme support */
        :root {
          --glass-bg-light: rgba(255, 255, 255, 0.15);
          --glass-bg-light-hover: rgba(255, 255, 255, 0.2);
          --glass-border-light: rgba(255, 255, 255, 0.2);
          --glass-border-light-hover: rgba(255, 255, 255, 0.3);
          --glass-shadow-light: 0 4px 20px rgba(31, 38, 135, 0.3);
          --glass-shadow-light-hover: 0 6px 25px rgba(31, 38, 135, 0.4);
          --glass-inset-light: inset 0 2px 10px rgba(255, 255, 255, 0.1);
          --glass-inset-light-hover: inset 0 3px 15px rgba(255, 255, 255, 0.15);
          
          --glass-bg-dark: linear-gradient(135deg, rgba(25, 25, 25, 0.95) 0%, rgba(15, 15, 15, 0.95) 100%);
          --glass-bg-dark-hover: linear-gradient(135deg, rgba(35, 35, 35, 0.98) 0%, rgba(20, 20, 20, 0.98) 100%);
          --glass-border-dark: rgba(60, 60, 60, 0.3);
          --glass-border-dark-hover: rgba(80, 80, 80, 0.4);
          --glass-shadow-dark: 0 8px 32px rgba(0, 0, 0, 0.8);
          --glass-shadow-dark-hover: 0 12px 40px rgba(0, 0, 0, 0.9);
          --glass-inset-dark: inset 0 1px 0 rgba(255, 255, 255, 0.05);
          --glass-inset-dark-hover: inset 0 1px 0 rgba(255, 255, 255, 0.08);
        }

        /* Base container */
        .voice-ai-bar-container {
          position: fixed;
          top: 50%;
          left: 50%;
          transform: translate(-50%, -50%);
          z-index: 1000;
          isolation: isolate;
        }

        /* Shared glass bar styles */
        .glass-bar-idle,
        .glass-bar-active,
        .glass-bar-input,
        .glass-bar-response,
        .glass-bar-response-width,
        .glass-bar-response-height,
        .glass-bar-response-summary,
        .glass-bar-response-expanding {
          position: relative;
          backdrop-filter: blur(20px) saturate(180%);
          border-radius: 1.5rem;
          display: flex;
          align-items: center;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
        }

        /* Theme-specific styles will be injected by the wrapper components */
        
        /* Rest of the styles remain the same... */
        .glass-mic-btn {
          width: 2rem;
          height: 2rem;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: transparent;
          border: none;
          cursor: pointer;
          position: relative;
          transition: all 0.2s;
          flex-shrink: 0;
        }

        .glass-mic-btn:hover {
          transform: scale(1.05);
        }

        .glass-mic-btn:active {
          transform: scale(0.95);
        }

        .input-form {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          width: 100%;
        }

        .glass-input {
          flex: 1;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: 1.25rem;
          padding: 0.375rem 1rem;
          color: white;
          font-size: 0.875rem;
          outline: none;
          transition: all 0.2s;
        }

        .glass-input:focus {
          background: rgba(255, 255, 255, 0.08);
          border-color: rgba(255, 255, 255, 0.2);
        }

        .glass-input::placeholder {
          color: rgba(255, 255, 255, 0.5);
        }

        .glass-send-btn {
          width: 2rem;
          height: 2rem;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(255, 255, 255, 0.1);
          border: 1px solid rgba(255, 255, 255, 0.2);
          cursor: pointer;
          transition: all 0.2s;
          flex-shrink: 0;
        }

        .glass-send-btn:hover:not(:disabled) {
          background: rgba(255, 255, 255, 0.15);
          border-color: rgba(255, 255, 255, 0.3);
          transform: scale(1.05);
        }

        .glass-send-btn:active:not(:disabled) {
          transform: scale(0.95);
        }

        .glass-send-btn:disabled {
          opacity: 0.3;
          cursor: not-allowed;
        }

        .status-text-container {
          flex: 1;
          min-width: 0;
          display: flex;
          align-items: center;
          height: 100%;
          position: relative;
          overflow: hidden;
        }

        .status-content {
          width: 100%;
          overflow: hidden;
        }

        .marquee-text {
          display: inline-block;
          padding-left: 100%;
        }

        .visualizer-container {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .sample-buttons {
          display: flex;
          gap: 0.25rem;
          opacity: 0;
          animation: fade-in 0.3s ease-out forwards;
        }

        .sample-btn {
          width: 1.75rem;
          height: 1.75rem;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          transition: all 0.2s;
          color: rgba(255, 255, 255, 0.6);
          opacity: 0;
          animation: fade-in 0.3s ease-out forwards;
        }

        .sample-btn:hover {
          background: rgba(255, 255, 255, 0.1);
          border-color: rgba(255, 255, 255, 0.2);
          color: white;
          transform: scale(1.1);
        }

        .sample-btn:active {
          transform: scale(0.95);
        }

        .close-btn {
          width: 1.75rem;
          height: 1.75rem;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          transition: all 0.2s;
          color: rgba(255, 255, 255, 0.6);
        }

        .close-btn:hover {
          background: rgba(255, 0, 0, 0.1);
          border-color: rgba(255, 0, 0, 0.2);
          color: #ff6b6b;
          transform: scale(1.1);
        }

        .response-expanded {
          display: flex;
          flex-direction: column;
          padding: 1rem;
          gap: 1rem;
          width: var(--expanded-width);
          max-height: var(--expanded-height);
          overflow-y: auto;
        }

        .response-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }

        .response-title {
          font-size: 1rem;
          font-weight: 600;
          color: white;
          margin: 0;
        }

        .response-samples {
          display: flex;
          gap: 0.5rem;
          align-items: center;
        }

        .response-content-wrapper {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .response-item {
          border-radius: 0.75rem;
          overflow: hidden;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .code-block,
        .image-block,
        .video-block,
        .text-block {
          display: flex;
          flex-direction: column;
        }

        .code-header,
        .image-header,
        .video-header,
        .text-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 0.75rem;
          background: rgba(255, 255, 255, 0.05);
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
          color: rgba(255, 255, 255, 0.8);
          font-size: 0.75rem;
          font-weight: 500;
        }

        .code-content {
          padding: 1rem;
          margin: 0;
          font-family: "Fira Code", "Monaco", monospace;
          font-size: 0.75rem;
          line-height: 1.5;
          color: rgba(255, 255, 255, 0.9);
          overflow-x: auto;
        }

        .image-container,
        .video-container {
          padding: 0.75rem;
        }

        .text-content {
          padding: 1rem;
          color: rgba(255, 255, 255, 0.9);
          font-size: 0.875rem;
          line-height: 1.6;
        }

        .copy-btn {
          width: 1.5rem;
          height: 1.5rem;
          border-radius: 0.375rem;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(255, 255, 255, 0.05);
          border: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          transition: all 0.2s;
          color: rgba(255, 255, 255, 0.6);
        }

        .copy-btn:hover {
          background: rgba(255, 255, 255, 0.1);
          border-color: rgba(255, 255, 255, 0.2);
          color: white;
          transform: scale(1.05);
        }

        .copy-btn:active {
          transform: scale(0.95);
        }

        @keyframes fade-in {
          from {
            opacity: 0;
            transform: scale(0.9);
          }
          to {
            opacity: 1;
            transform: scale(1);
          }
        }

        .response-content-calculator {
          position: absolute;
          top: -9999px;
          left: -9999px;
          visibility: hidden;
          width: auto;
          height: auto;
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
        }

        .audio-visualizer-wrapper {
          display: flex;
          align-items: center;
          justify-content: center;
          position: relative;
          flex-shrink: 0;
          width: 60px;
          height: 100%;
        }

        .status-text-wrapper {
          display: flex;
          align-items: center;
          flex: 1;
          min-width: 0;
          height: 100%;
        }

        .state-message-container {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          width: 100%;
          height: 100%;
          padding: 0 0.25rem;
        }

        .state-icon-wrapper {
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
        }

        .state-text-wrapper {
          display: flex;
          align-items: center;
          flex: 1;
          min-width: 0;
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
        
        ${theme === 'dark' ? `
        /* Dark theme specific transitions */
        .text-transitioning {
          animation: text-fade 0.3s ease-in-out;
        }

        @keyframes text-fade {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.3; }
        }
        ` : ''}
      `}} />
    </div>
  );
}