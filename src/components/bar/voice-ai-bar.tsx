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
import AudioVisualizer from "./audio-visualizer";

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
  DEFAULT_WIDTH: 160,
  DEFAULT_HEIGHT: 40,
  EXPANDED_WIDTH: 320,
  EXPANDED_HEIGHT: 50,
};

/**
 * Component name for backend interactions - MUST match backend element handling
 */
const COMPONENT_ID = "voice-ai-bar";

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

  // Legacy state for visual appearance (derived from barState)
  const [responseContent, setResponseContent] = useState<ResponseContent[]>([]);
  const [isExpanded, setIsExpanded] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const [responsePhase, setResponsePhase] = useState<
    "collapsed" | "expanding-width" | "expanding-height" | "showing-content"
  >("collapsed");
  const [isIdleHovered, setIsIdleHovered] = useState(false);
  const [marqueeKey, setMarqueeKey] = useState(0);

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
            console.log("📨 VoiceAIBar: Received state update:", event.payload);

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
                "❌ VoiceAIBar: Invalid state data received:",
                payload
              );
            }
          }
        );

        console.log("✅ VoiceAIBar: Event listener established");
      } catch (error) {
        console.error("❌ VoiceAIBar: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        console.log("🔄 VoiceAIBar: Event listener cleaned up");
      }
    };
  }, []);

  // === WINDOW RESIZING LOGIC ===

  /**
   * Responsive window resizing based on UI state
   * Compact states use small dimensions, expanded states use larger dimensions
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
          `🔧 VoiceAIBar: Resizing window to ${currentWidth}x${currentHeight} for state: ${currentUiState}`
        );

        await appWindow.setSize(new LogicalSize(currentWidth, currentHeight));
      } catch (error) {
        console.error("❌ VoiceAIBar: Failed to resize window:", error);
      }
    };

    resizeWindow();
  }, [barState.barState]);

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
      console.log("🔧 VoiceAIBar: Sending interaction:", interaction);

      await invoke("ui_handle_interaction", {
        elementId: COMPONENT_ID,
        interaction,
      });

      console.log("✅ VoiceAIBar: Interaction sent successfully");
    } catch (error) {
      console.error("❌ VoiceAIBar: Interaction failed:", error);
    }
  };

  /**
   * Local input state management - No backend interaction needed
   * The component manages its own input state and only sends the final value on submit
   */
  const [localInputValue, setLocalInputValue] = useState("");

  /**
   * Handle input changes locally - No backend interaction
   * This is more efficient than broadcasting every keystroke
   */
  const handleInputChange = useCallback((value: string) => {
    setLocalInputValue(value);
    // No backend interaction needed for input changes
  }, []);

  /**
   * Sync local input state with backend state updates
   * This ensures the input reflects the backend's current state
   */
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

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

  /**
   * Handle bar click - Demonstrates simple interaction
   */
  const handleClick = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_CLICK);
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle form submission - Demonstrates validation + interaction
   */
  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const trimmedValue = localInputValue.trim();

      if (trimmedValue) {
        const interaction = createInteraction(UI.INTERACTION_TYPES_SUBMIT, {
          value: trimmedValue,
        });
        await sendInteraction(interaction);
      } else {
        console.log("⚠️ VoiceAIBar: Empty submission ignored");
      }
    },
    [localInputValue]
  );

  /**
   * Handle focus events - Demonstrates state-aware interactions
   */
  const handleFocus = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_FOCUS, {
      is_focused: true,
    });
    await sendInteraction(interaction);
  }, []);

  /**
   * Handle blur events - Demonstrates state-aware interactions
   */
  const handleBlur = useCallback(async () => {
    const interaction = createInteraction(UI.INTERACTION_TYPES_BLUR, {
      is_focused: false,
    });
    await sendInteraction(interaction);
  }, []);

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

      setTimeout(() => {
        setResponsePhase("expanding-width");

        setTimeout(() => {
          setResponsePhase("expanding-height");

          setTimeout(() => {
            setResponsePhase("showing-content");

            setTimeout(() => {
              setIsExpanded(false);
            }, 50);
          }, 500);
        }, 400);
      }, 100);
    }, 50);
  };

  // Handle response state transitions and marquee reset
  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_AGENT_RESPONDING) {
      // Set sample response content when entering response state
      setResponseContent([
        sampleResponses.text,
        sampleResponses.code,
        sampleResponses.component,
      ]);
      handleResponseState();
    }
    // Reset marquee when state changes
    setMarqueeKey((prev) => prev + 1);
  }, [barState.barState]);

  // === UI STATE CALCULATIONS ===

  const toggleListening = useCallback(async () => {
    const interaction = createInteraction("toggle_listening");
    await sendInteraction(interaction);
  }, []);

  const handleInputSubmit = useCallback(
    async (e?: React.FormEvent) => {
      if (e) e.preventDefault();
      await handleSubmit(e!);
    },
    [handleSubmit]
  );

  const toggleInputMode = useCallback(async () => {
    const interaction = createInteraction("toggle_input");
    await sendInteraction(interaction);
  }, []);

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
  }, []);

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
    const currentUiState = barState.barState;
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
    const currentUiState = barState.barState;
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
    const currentUiState = barState.barState;
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
        {barState.barState === UI.BAR_STATES_INPUT && (
          <form onSubmit={handleInputSubmit} className="input-form">
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => handleInputChange(e.target.value)}
              onFocus={handleFocus}
              onBlur={handleBlur}
              placeholder="Type your request..."
              aria-label="Assistant input"
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
        {barState.barState === UI.BAR_STATES_AGENT_RESPONDING &&
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
        {barState.barState === UI.BAR_STATES_AGENT_RESPONDING &&
          responsePhase === "showing-content" &&
          !isCalculatingDimensions && (
            <div className="response-container">
              {/* Compact view when collapsed */}
              {!isExpanded && (
                <button
                  type="button"
                  className="response-summary bg-transparent p-0 m-0 border-0"
                  onClick={toggleExpanded}
                  aria-label="Expand AI response"
                >
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
                </button>
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
                      type="button"
                      onClick={closeResponse}
                      className="close-response-btn"
                      aria-label="Close AI response"
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
        {![
          UI.BAR_STATES_INPUT,
          UI.BAR_STATES_AGENT_RESPONDING,
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_ERROR,
          UI.BAR_STATES_SUCCESS,
        ].includes(barState.barState as any) && (
          <div className="visualizer-status-container">
            {/* Audio Visualizer */}
            <div className="audio-visualizer-wrapper">
              <div className="audio-visualizer-content visible">
                <AudioVisualizer
                  appState={
                    barState.barState === UI.BAR_STATES_LISTENING
                      ? "listening"
                      : "processing"
                  }
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
                  speed={30}
                  gradient={true}
                  gradientColor="rgba(255, 255, 255, 0)"
                  gradientWidth={8}
                  pauseOnHover={true}
                  delay={1.5}
                  play={barState.barState !== UI.BAR_STATES_DEFAULT}
                >
                  <span className="marquee-text text-white/80 text-xs whitespace-nowrap pr-12">
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
              <span className="state-message text-white/90 text-xs">
                {currentMessage}
              </span>
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
        {barState.barState === UI.BAR_STATES_DEFAULT && (
          <div
            className="idle-container"
            onMouseEnter={() => setIsIdleHovered(true)}
            onMouseLeave={() => setIsIdleHovered(false)}
            onClick={
              [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
                barState.barState as any
              )
                ? handleClick
                : undefined
            }
            role="button"
            tabIndex={0}
            aria-label="Activate assistant"
            onKeyDown={(e) => {
              if (
                (e.key === "Enter" || e.key === " ") &&
                [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_DICTATION_READY].includes(
                  barState.barState as any
                )
              ) {
                e.preventDefault();
                handleClick();
              }
            }}
          >
            <div
              className={`idle-waveform ${
                !isIdleHovered ? "visible" : "hidden"
              }`}
            >
              <AudioVisualizer
                appState="idle"
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
                aria-label="Start voice assistant"
              >
                <div className="icon-container">{getStateIcon()}</div>
              </button>

              <button
                onClick={toggleInputMode}
                className="glass-keyboard-btn"
                aria-label="Use text input"
              >
                <Keyboard className="w-3 h-3 text-white/70" />
              </button>
            </div>
          </div>
        )}

        {![
          UI.BAR_STATES_DEFAULT,
          UI.BAR_STATES_INPUT,
          UI.BAR_STATES_AGENT_RESPONDING,
        ].includes(barState.barState as any) && (
          <button
            onClick={toggleListening}
            className="glass-mic-btn"
            disabled={[
              UI.BAR_STATES_LOADING,
              UI.BAR_STATES_SUBMITTING,
            ].includes(barState.barState as any)}
          >
            <div className="icon-container">{getStateIcon()}</div>
          </button>
        )}

        {barState.barState === UI.BAR_STATES_INPUT && (
          <button onClick={toggleInputMode} className="glass-mic-btn close-btn">
            <div className="icon-container">{getStateIcon()}</div>
          </button>
        )}
      </div>

      <style>{`
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
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.4rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: center;
          justify-content: center;
          width: 160px;
          height: 40px;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
          cursor: pointer;
        }

        .glass-bar-idle:hover {
          background: rgba(255, 255, 255, 0.9);
          border-color: rgba(255, 255, 255, 0.7);
          box-shadow: 0 6px 25px rgba(31, 38, 135, 0.4),
            inset 0 3px 15px rgba(255, 255, 255, 0.15);
        }

        .glass-bar-active {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem 0.75rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: center;
          gap: 0.75rem;
          width: 280px;
          height: 40px;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
        }

        .glass-bar-input {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem 0.75rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: center;
          gap: 0.75rem;
          width: 320px;
          height: 40px;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
        }

        .glass-bar-response {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem 0.75rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: center;
          gap: 0.75rem;
          width: 320px;
          height: 40px;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          overflow: hidden;
        }

        .glass-bar-response-width {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem 0.75rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: center;
          gap: 0.75rem;
          width: var(--response-width, 360px);
          height: 40px;
          transition: width 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          overflow: hidden;
          will-change: width;
          transform: translateZ(0);
        }

        .glass-bar-response-height {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: flex-start;
          gap: 0.75rem;
          width: var(--response-width, 360px);
          height: var(--summary-height, 60px);
          transition: height 0.6s cubic-bezier(0.23, 1, 0.32, 1);
          overflow: hidden;
          will-change: height;
          transform: translateZ(0);
        }

        .glass-bar-response-summary {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: flex-start;
          gap: 0.75rem;
          width: var(--response-width, 360px);
          height: var(--summary-height, 60px);
          transition: height 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          overflow: hidden;
          will-change: height;
          transform: translateZ(0);
        }

        .glass-bar-response-expanding {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
          display: flex;
          align-items: flex-start;
          gap: 0.75rem;
          width: var(--response-width, 360px);
          height: var(--response-height, 120px);
          transition: height 0.5s cubic-bezier(0.25, 0.46, 0.45, 0.94);
          overflow: hidden;
          will-change: height;
          transform: translateZ(0);
        }

        @keyframes fade-in {
          from {
            opacity: 0;
            transform: translateY(4px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        @keyframes fade-in-up {
          from {
            opacity: 0;
            transform: translateY(8px) scale(0.98);
          }
          to {
            opacity: 1;
            transform: translateY(0) scale(1);
          }
        }

        .animate-fade-in {
          animation: fade-in 0.3s cubic-bezier(0.4, 0, 0.2, 1) both;
          will-change: opacity, transform;
        }

        .animate-fade-in-up {
          animation: fade-in-up 0.5s cubic-bezier(0.25, 0.46, 0.45, 0.94) both;
          will-change: opacity, transform;
        }

        .glass-bar-response-width,
        .glass-bar-response-height,
        .glass-bar-response,
        .glass-bar-response-expanded {
          will-change: width, height;
          transform: translateZ(0);
        }

        .glass-bar-response-expanded {
          position: relative;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.5);
          border-radius: 1.5rem;
          padding: 0.5rem;
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.6),
            inset 0 2px 10px rgba(255, 255, 255, 0.3);
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

        .response-content {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
          padding: 0.75rem;
          overflow-y: auto;
          max-height: calc(70vh - 3rem);
          scroll-behavior: smooth;
          -webkit-overflow-scrolling: touch;
        }

        .response-item {
          width: 100%;
          contain: layout style;
          opacity: 0;
        }

        .response-item.animate-fade-in-up {
          opacity: 1;
        }

        .glass-bar-idle::after,
        .glass-bar-active::after,
        .glass-bar-input::after,
        .glass-bar-response::after,
        .glass-bar-response-expanded::after {
          content: "";
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          background: rgba(255, 255, 255, 0.15);
          border-radius: 1.5rem;
          backdrop-filter: blur(1px);
          box-shadow: inset -6px -4px 0px -7px rgba(255, 255, 255, 0.5),
            inset 0px -5px 0px -4px rgba(255, 255, 255, 0.4);
          opacity: 0.6;
          z-index: -1;
          pointer-events: none;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
        }

        .glass-mic-btn {
          position: relative;
          width: 2rem;
          height: 2rem;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(15px);
          border: 1px solid rgba(255, 255, 255, 0.6);
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
          box-shadow: 0 2px 10px rgba(31, 38, 135, 0.3),
            inset 0 1px 5px rgba(255, 255, 255, 0.1);
          flex-shrink: 0;
        }

        .glass-keyboard-btn {
          position: relative;
          width: 2rem;
          height: 2rem;
          background: rgba(255, 255, 255, 0.85);
          backdrop-filter: blur(15px);
          border: 1px solid rgba(255, 255, 255, 0.6);
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
          box-shadow: 0 2px 10px rgba(31, 38, 135, 0.3),
            inset 0 1px 5px rgba(255, 255, 255, 0.1);
          flex-shrink: 0;
        }

        .glass-mic-btn:hover,
        .glass-keyboard-btn:hover {
          background: rgba(255, 255, 255, 0.95);
          transform: scale(1.1);
          box-shadow: 0 4px 15px rgba(31, 38, 135, 0.4);
        }

        .glass-mic-btn.close-btn {
          background: rgba(255, 255, 255, 0.9);
        }

        .glass-mic-btn.close-btn:hover {
          background: rgba(255, 255, 255, 0.3);
        }

        .glass-mic-btn.expand-btn {
          background: rgba(79, 70, 229, 0.3);
          border-color: rgba(79, 70, 229, 0.5);
        }

        .glass-mic-btn.expand-btn:hover {
          background: rgba(79, 70, 229, 0.4);
        }

        .icon-container {
          transition: all 0.3s ease-in-out;
        }

        .input-form {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          flex: 1;
          width: 100%;
        }

        .glass-input {
          flex: 1;
          background: rgba(255, 255, 255, 0.1);
          border: none;
          border-radius: 1rem;
          padding: 0.25rem 0.75rem;
          color: white;
          font-size: 0.875rem;
          outline: none;
          transition: all 0.3s ease;
        }

        .glass-input::placeholder {
          color: rgba(255, 255, 255, 0.5);
        }

        .glass-input:focus {
          background: rgba(255, 255, 255, 0.85);
          box-shadow: 0 0 0 2px rgba(124, 58, 237, 0.3);
        }

        .glass-send-btn {
          width: 1.5rem;
          height: 1.5rem;
          background: rgba(124, 58, 237, 0.6);
          border: none;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          transition: all 0.3s ease;
          flex-shrink: 0;
        }

        .glass-send-btn:hover {
          background: rgba(124, 58, 237, 0.8);
          transform: scale(1.1);
        }

        .glass-send-btn:disabled {
          background: rgba(124, 58, 237, 0.3);
          cursor: not-allowed;
          transform: scale(1);
        }

        .response-container {
          display: flex;
          flex: 1;
          width: 100%;
          height: 100%;
          overflow: hidden;
        }

        .response-summary {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          width: 100%;
          cursor: pointer;
          padding: 0.25rem 0;
        }

        .response-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          color: rgba(255, 255, 255, 0.8);
        }

        .response-preview {
          flex: 1;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .response-title {
          font-size: 0.875rem;
          color: rgba(255, 255, 255, 0.9);
          font-weight: 500;
        }

        .response-expanded {
          display: flex;
          flex-direction: column;
          width: 100%;
          height: 100%;
          max-height: 70vh;
        }

        .response-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0.5rem 0.75rem;
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }

        .response-header h3 {
          font-size: 0.875rem;
          font-weight: 500;
          color: rgba(255, 255, 255, 0.9);
        }

        .close-response-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 1.5rem;
          height: 1.5rem;
          border-radius: 50%;
          background: rgba(255, 255, 255, 0.1);
          border: none;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .close-response-btn:hover {
          background: rgba(255, 255, 255, 0.9);
        }

        .response-content {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
          padding: 0.75rem;
          overflow-y: auto;
          max-height: calc(70vh - 4rem);
          scroll-behavior: smooth;
          -webkit-overflow-scrolling: touch;
          scrollbar-width: thin;
          scrollbar-color: rgba(255, 255, 255, 0.3) transparent;
        }

        .response-content::-webkit-scrollbar {
          width: 6px;
        }

        .response-content::-webkit-scrollbar-track {
          background: transparent;
        }

        .response-content::-webkit-scrollbar-thumb {
          background: rgba(255, 255, 255, 0.3);
          border-radius: 3px;
        }

        .response-content::-webkit-scrollbar-thumb:hover {
          background: rgba(255, 255, 255, 0.5);
        }

        .text-block,
        .code-block,
        .image-block,
        .video-block {
          background: rgba(255, 255, 255, 0.15);
          border-radius: 0.75rem;
          overflow: hidden;
          border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .text-header,
        .code-header,
        .image-header,
        .video-header {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.75rem;
          font-size: 0.75rem;
          color: rgba(255, 255, 255, 0.7);
          background: rgba(0, 0, 0, 0.1);
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }

        .code-header {
          display: flex;
          justify-content: space-between;
        }

        .copy-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.25rem;
          border-radius: 0.25rem;
          background: rgba(255, 255, 255, 0.1);
          border: none;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .copy-btn:hover {
          background: rgba(255, 255, 255, 0.9);
        }

        .text-content {
          padding: 0.75rem;
          font-size: 0.875rem;
          color: rgba(255, 255, 255, 0.9);
          line-height: 1.5;
        }

        .code-content {
          padding: 0.75rem;
          font-size: 0.75rem;
          color: rgba(255, 255, 255, 0.9);
          line-height: 1.5;
          font-family: monospace;
          white-space: pre-wrap;
          overflow-x: auto;
          background: rgba(0, 0, 0, 0.2);
        }

        .image-container {
          width: 100%;
          overflow: hidden;
        }

        .image-container img {
          width: 100%;
          height: auto;
          object-fit: contain;
        }

        .video-container {
          width: 100%;
          overflow: hidden;
        }

        .video-container video {
          width: 100%;
          height: auto;
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

        .state-error {
          animation: pulse-border-red 1.5s ease-in-out infinite,
            shake 0.5s ease-in-out;
          background: rgba(239, 68, 68, 0.1) !important;
        }

        .state-success {
          animation: pulse-border-green 1.5s ease-in-out infinite,
            bounce 0.6s ease-in-out;
          background: rgba(16, 185, 129, 0.1) !important;
        }

        .state-listening {
          border-color: rgba(59, 130, 246, 0.6);
          background: rgba(59, 130, 246, 0.05);
        }

        .state-processing {
          border-color: rgba(168, 85, 247, 0.6);
          background: rgba(168, 85, 247, 0.05);
        }

        .state-speaking {
          border-color: rgba(34, 197, 94, 0.6);
          background: rgba(34, 197, 94, 0.05);
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

        @media (max-width: 640px) {
          .glass-bar-active {
            padding: 0.4rem 0.6rem;
            gap: 0.5rem;
            width: 240px;
          }

          .glass-bar-input,
          .glass-bar-response {
            width: 300px;
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
      `}</style>
    </div>
  );
}
