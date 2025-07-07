"use client";

import type React from "react";
import { useState, useEffect, useRef, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Mic,
  MicOff,
  Zap,
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
} from "lucide-react";
import Marquee from "react-fast-marquee";
import AudioVisualizer, { type AppState } from "./audio-visualizer";
import type {
  VoiceAIBarProps,
  AssistantState,
  ContentType,
  ResponseContent,
} from "../../types/voice-ai";

// === UI API TYPES ===
// These types match the backend BarState exactly
type UIState =
  | "default"
  | "expanding"
  | "input"
  | "shrinking"
  | "submitting"
  | "loading"
  | "success"
  | "error"
  | "speaking"
  | "listening"
  | "transcribing"
  | "dictating"
  | "dictation_ready"
  | "always_listening"
  | "finishing"
  | "agent_responding";

interface UIStateData {
  uiState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  spokenText: string;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  audioLevel: number;
  voiceMode: string;
  agentState: string | null;
  currentTransitionId: string | null;
}

// === UTILITY FUNCTIONS ===
const mapAssistantStateToUIState = (state: AssistantState): UIState => {
  switch (state) {
    case "idle":
      return "default";
    case "listening":
      return "listening";
    case "processing":
      return "loading";
    case "speaking":
      return "speaking";
    case "error":
      return "error";
    case "success":
      return "success";
    case "input":
      return "input";
    case "response":
      return "agent_responding";
    default:
      return "default";
  }
};

const mapUIStateToAssistantState = (state: UIState): AssistantState => {
  switch (state) {
    case "default":
    case "shrinking":
      return "idle";
    case "listening":
    case "transcribing":
      return "listening";
    case "loading":
    case "submitting":
    case "finishing":
      return "processing";
    case "speaking":
      return "speaking";
    case "error":
      return "error";
    case "success":
      return "success";
    case "input":
    case "expanding":
      return "input";
    case "agent_responding":
      return "response";
    default:
      return "idle";
  }
};

// Convert AssistantState to AppState for AudioVisualizer
const mapAssistantStateToAppState = (state: AssistantState): AppState => {
  switch (state) {
    case "idle":
      return "idle";
    case "listening":
      return "listening";
    case "processing":
      return "processing";
    case "speaking":
      return "speaking";
    case "error":
      return "error";
    case "success":
      return "success";
    case "input":
      return "idle"; // Input mode should show idle state in visualizer
    case "response":
      return "speaking"; // Response mode should show speaking state in visualizer
    default:
      return "idle";
  }
};

export function VoiceAIBar({
  onStateChange,
  initialState = "idle",
  className = "",
  sampleResponses: propSampleResponses,
}: VoiceAIBarProps) {
  // === UI API STATE ===
  const [uiState, setUIState] = useState<UIState>(
    mapAssistantStateToUIState(initialState)
  );
  const [uiStateData, setUIStateData] = useState<UIStateData | null>(null);

  // === LOCAL STATE (for UI only) ===
  const [currentMessage, setCurrentMessage] = useState("");
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [showStateIcon, setShowStateIcon] = useState(false);
  const [inputText, setInputText] = useState("");
  const [responseContent, setResponseContent] = useState<ResponseContent[]>([]);
  const [isExpanded, setIsExpanded] = useState(false);
  const [responsePhase, setResponsePhase] = useState<
    "collapsed" | "expanding-width" | "expanding-height" | "showing-content"
  >("collapsed");
  const [marqueeKey, setMarqueeKey] = useState(0);
  const [textTransitioning, setTextTransitioning] = useState(false);
  const [heightTransitionTarget, setHeightTransitionTarget] = useState<
    "collapsed" | "summary" | "expanded"
  >("collapsed");

  // === REFS ===
  const inputRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  // === DERIVED STATE ===
  const assistantState = mapUIStateToAssistantState(uiState);
  const visualizerState = mapAssistantStateToAppState(assistantState);

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

  // Messages for different states
  const stateMessages: Record<AssistantState, string> = {
    idle: "Ready",
    listening: "Listening to your request...",
    processing:
      "Processing your request, please wait while I analyze your input...",
    speaking:
      "Here's what I found for you based on your request and current context...",
    error:
      "Sorry, I couldn't understand that request. Please try speaking more clearly.",
    success:
      "Task completed successfully! Is there anything else I can help you with today?",
    input: "Type your request...",
    response: "Here's what I found:",
    dictating: "Dictating...",
    thinking: "Thinking...",
    responding: "Responding...",
    finished: "Finished.",
    failed: "Failed.",
    cancelled: "Cancelled.",
    offline: "Offline.",
  };

  // === UI API EVENT LISTENERS ===
  useEffect(() => {
    let unlisten: UnlistenFn;

    const setupEventListener = async () => {
      try {
        unlisten = await listen("bar-state-update", (event) => {
          const stateData = event.payload as UIStateData;
          console.log("VoiceAIBar: Received state update:", stateData);

          setUIStateData(stateData);
          setUIState(stateData.uiState);
          setInputText(stateData.inputValue);

          // Update current message based on state
          const newAssistantState = mapUIStateToAssistantState(
            stateData.uiState
          );
          setCurrentMessage(stateMessages[newAssistantState]);

          // Notify parent of state change
          onStateChange?.(newAssistantState);
        });
      } catch (error) {
        console.error("VoiceAIBar: Failed to setup event listener:", error);
      }
    };

    setupEventListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [onStateChange, stateMessages]);

  // === UI API COMMAND HELPERS ===
  const handleInteraction = useCallback(async (type: string, data?: any) => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "voice-ai-bar",
        interaction: {
          element_id: "voice-ai-bar",
          interaction_type: type,
          data: data || {},
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error(`VoiceAIBar: UI interaction failed (${type}):`, error);
    }
  }, []);

  // === STATE CHANGE HANDLER ===
  const changeState = useCallback(
    (newState: AssistantState) => {
      if (newState === assistantState) return;

      const newUIState = mapAssistantStateToUIState(newState);

      // Start text fade out
      setTextTransitioning(true);

      // After fade out completes, update message and fade in
      setTimeout(() => {
        setCurrentMessage(stateMessages[newState]);

        // Reset marquee when state changes
        setMarqueeKey((prev) => prev + 1);

        // Send interaction to backend
        handleInteraction("state_change", { newState: newUIState });

        // End text transition (fade in)
        setTimeout(() => {
          setTextTransitioning(false);
        }, 150);
      }, 150);

      // Handle state-specific transitions
      if (newState === "input") {
        setTimeout(() => {
          setIsTransitioning(true);
          setTimeout(() => {
            inputRef.current?.focus();
            setIsTransitioning(false);
          }, 300);
        }, 300);
      } else if (newState === "response") {
        setTimeout(() => {
          setIsTransitioning(true);
          handleResponseState();
          setTimeout(() => {
            setIsTransitioning(false);
          }, 1200);
        }, 300);
      } else {
        setTimeout(() => {
          setIsTransitioning(true);
          setTimeout(() => {
            setIsTransitioning(false);
          }, 200);
        }, 300);
      }

      // Show state icon for success/error states
      if (newState === "success" || newState === "error") {
        setTimeout(() => {
          setShowStateIcon(true);
          setTimeout(() => {
            setShowStateIcon(false);
          }, 1500);
        }, 300);
      }
    },
    [assistantState, stateMessages, handleInteraction]
  );

  // === RESPONSE STATE HANDLER ===
  const handleResponseState = useCallback(() => {
    setResponsePhase("expanding-width");

    setTimeout(() => {
      setResponsePhase("expanding-height");
      setTimeout(() => {
        setResponsePhase("showing-content");
        setHeightTransitionTarget("summary");
      }, 300);
    }, 300);
  }, []);

  // === EVENT HANDLERS ===
  const toggleListening = useCallback(() => {
    if (assistantState === "listening") {
      handleInteraction("stop_listening");
    } else {
      handleInteraction("start_listening");
    }
  }, [assistantState, handleInteraction]);

  const handleInputSubmit = useCallback(
    (e?: React.FormEvent) => {
      if (e) {
        e.preventDefault();
      }

      const userInput = inputText.trim();
      if (userInput) {
        handleInteraction("submit", { query: userInput });
        setInputText("");
        changeState("processing");

        // Mock response generation (in real app, this would come from backend)
        setTimeout(() => {
          let responseItems: ResponseContent[] = [];

          if (
            userInput.toLowerCase().includes("glass") ||
            userInput.toLowerCase().includes("design")
          ) {
            responseItems = [
              sampleResponses.text,
              sampleResponses.code,
              sampleResponses.component,
            ];
          } else if (
            userInput.toLowerCase().includes("code") ||
            userInput.toLowerCase().includes("css")
          ) {
            responseItems = [sampleResponses.code, sampleResponses.component];
          } else if (
            userInput.toLowerCase().includes("image") ||
            userInput.toLowerCase().includes("example")
          ) {
            responseItems = [sampleResponses.image, sampleResponses.text];
          } else {
            responseItems = [sampleResponses.text];
          }

          setResponseContent(responseItems);
          changeState("response");

          setTimeout(() => {
            setIsExpanded(true);
          }, 200);
        }, 800);
      }
    },
    [inputText, handleInteraction, changeState, sampleResponses]
  );

  const toggleInputMode = useCallback(() => {
    if (assistantState === "input") {
      handleInteraction("blur");
    } else {
      handleInteraction("focus");
    }
  }, [assistantState, handleInteraction]);

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

  const closeResponse = useCallback(() => {
    setIsExpanded(false);
    setResponsePhase("collapsed");
    setTimeout(() => {
      changeState("idle");
    }, 300);
  }, [changeState]);

  const copyToClipboard = useCallback((text: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => {
        console.log("Copied to clipboard!");
      })
      .catch((err) => {
        console.error("Failed to copy: ", err);
      });
  }, []);

  // === FOCUS MANAGEMENT ===
  useEffect(() => {
    if (uiState === "input" && inputRef.current) {
      inputRef.current.focus();
    }
  }, [uiState]);

  // === INPUT CHANGE HANDLER ===
  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      setInputText(value);
      handleInteraction("input_change", { value });
    },
    [handleInteraction]
  );

  // === MAIN CLICK HANDLER ===
  const handleMainClick = useCallback(() => {
    if (assistantState === "response") {
      toggleExpanded();
    } else if (assistantState === "input") {
      // Handle input submission on click
      handleInputSubmit();
    } else {
      handleInteraction("click");
    }
  }, [assistantState, toggleExpanded, handleInputSubmit, handleInteraction]);

  // Get icon based on current state
  const getStateIcon = () => {
    switch (assistantState) {
      case "listening":
        return (
          <Mic className="w-3 h-3 text-white transition-all duration-300" />
        );
      case "processing":
        return (
          <Zap className="w-3 h-3 text-white transition-all duration-300" />
        );
      case "speaking":
        return (
          <Volume2 className="w-3 h-3 text-white transition-all duration-300" />
        );
      case "error":
        return (
          <AlertCircle className="w-3 h-3 text-white transition-all duration-300" />
        );
      case "success":
        return (
          <CheckCircle className="w-3 h-3 text-white transition-all duration-300" />
        );
      case "input":
        return <X className="w-3 h-3 text-white transition-all duration-300" />;
      case "response":
        return isExpanded ? (
          <ChevronDown className="w-3 h-3 text-white transition-all duration-300" />
        ) : (
          <ChevronUp className="w-3 h-3 text-white transition-all duration-300" />
        );
      default:
        return (
          <MicOff className="w-3 h-3 text-white/70 transition-all duration-300" />
        );
    }
  };

  // Get the state feedback icon for waveform replacement
  const getStateFeedbackIcon = () => {
    switch (assistantState) {
      case "success":
        return (
          <CheckCircle className="w-4 h-4 text-green-400 animate-bounce" />
        );
      case "error":
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
      assistantState === "idle" ? "glass-bar-idle" : "glass-bar-active";

    if (assistantState === "input") {
      baseClass = "glass-bar-input";
    }

    if (assistantState === "response") {
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
    switch (assistantState) {
      case "listening":
        return baseClass + " state-listening";
      case "processing":
        return baseClass + " state-processing";
      case "speaking":
        return baseClass + " state-speaking";
      case "error":
        return baseClass + " state-error";
      case "success":
        return baseClass + " state-success";
      case "input":
        return baseClass + " state-input";
      case "response":
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
              <video
                src={item.content}
                controls
                className="w-full h-auto rounded-lg"
              />
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
            <div className="text-content">
              <p>{item.content}</p>
            </div>
          </div>
        );
    }
  };

  // === RENDER ===
  return (
    <div className={`voice-ai-bar ${getBarClass()} ${className}`}>
      <div className="bar-container">
        {/* Main Bar */}
        <div className="bar-main" onClick={handleMainClick}>
          {/* Left Section - State Icon */}
          <div className="bar-icon">{getStateIcon()}</div>

          {/* Center Section - Message/Input */}
          <div className="bar-center">
            {assistantState === "input" ? (
              <form onSubmit={handleInputSubmit} className="bar-input-form">
                <input
                  ref={inputRef}
                  type="text"
                  value={inputText}
                  onChange={handleInputChange}
                  onBlur={() => handleInteraction("blur")}
                  onFocus={() => handleInteraction("focus")}
                  placeholder="Type your request..."
                  className="bar-input"
                />
                <button
                  type="submit"
                  className="bar-submit"
                  disabled={!inputText.trim()}
                >
                  <Send className="w-3 h-3" />
                </button>
              </form>
            ) : (
              <div className="bar-message">
                {textTransitioning ? (
                  <div className="text-fade-out">{currentMessage}</div>
                ) : (
                  <Marquee
                    key={marqueeKey}
                    speed={30}
                    gradient={false}
                    pauseOnHover={true}
                    className="bar-marquee"
                  >
                    {currentMessage}
                  </Marquee>
                )}
              </div>
            )}
          </div>

          {/* Right Section - Audio Visualizer or State Icon */}
          <div className="bar-right">
            {showStateIcon ? (
              getStateFeedbackIcon()
            ) : (
              <AudioVisualizer
                appState={visualizerState}
                width={24}
                height={24}
                enableMicrophone={false}
                intensity={uiStateData?.audioLevel || 0.6}
                animationStyle="minimal"
                className="bar-visualizer"
              />
            )}
          </div>
        </div>

        {/* Response Content */}
        {assistantState === "response" && responseContent.length > 0 && (
          <div className="response-container">
            <div className="response-header">
              <div className="response-title">
                <span>AI Response</span>
                <div className="response-actions">
                  <button
                    onClick={toggleExpanded}
                    className="response-toggle"
                    aria-label={isExpanded ? "Collapse" : "Expand"}
                  >
                    {isExpanded ? (
                      <ChevronUp className="w-4 h-4" />
                    ) : (
                      <ChevronDown className="w-4 h-4" />
                    )}
                  </button>
                  <button
                    onClick={closeResponse}
                    className="response-close"
                    aria-label="Close response"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>
            </div>

            <div
              ref={contentRef}
              className={`response-content ${
                isExpanded ? "expanded" : "collapsed"
              }`}
            >
              {responseContent.map((item, index) => (
                <div key={index} className="response-item">
                  {renderContent(item)}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Quick Actions */}
        <div className="bar-actions">
          <button
            onClick={toggleListening}
            className={`action-btn ${
              assistantState === "listening" ? "active" : ""
            }`}
            aria-label={
              assistantState === "listening"
                ? "Stop listening"
                : "Start listening"
            }
          >
            {assistantState === "listening" ? (
              <Mic className="w-4 h-4" />
            ) : (
              <MicOff className="w-4 h-4" />
            )}
          </button>

          <button
            onClick={toggleInputMode}
            className={`action-btn ${
              assistantState === "input" ? "active" : ""
            }`}
            aria-label={
              assistantState === "input"
                ? "Exit input mode"
                : "Enter input mode"
            }
          >
            <Keyboard className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Styling (unchanged from original) */}
      <style>{`
        .voice-ai-bar {
          position: fixed;
          bottom: 20px;
          left: 50%;
          transform: translateX(-50%);
          z-index: 1000;
          transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
        }

        .bar-container {
          position: relative;
          display: flex;
          flex-direction: column;
          gap: 8px;
        }

        .bar-main {
          display: flex;
          align-items: center;
          gap: 12px;
          padding: 8px 16px;
          background: rgba(255, 255, 255, 0.1);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.2);
          border-radius: 25px;
          cursor: pointer;
          transition: all 0.3s ease;
          min-width: 200px;
        }

        .bar-main:hover {
          background: rgba(255, 255, 255, 0.15);
          transform: translateY(-1px);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
        }

        .bar-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 24px;
          height: 24px;
        }

        .bar-center {
          flex: 1;
          display: flex;
          align-items: center;
          min-height: 24px;
        }

        .bar-message {
          width: 100%;
          color: white;
          font-size: 14px;
          font-weight: 500;
        }

        .bar-marquee {
          width: 100%;
        }

        .text-fade-out {
          opacity: 0.5;
          transition: opacity 0.15s ease;
        }

        .bar-input-form {
          display: flex;
          align-items: center;
          gap: 8px;
          width: 100%;
        }

        .bar-input {
          flex: 1;
          background: transparent;
          border: none;
          outline: none;
          color: white;
          font-size: 14px;
          placeholder-color: rgba(255, 255, 255, 0.6);
        }

        .bar-input::placeholder {
          color: rgba(255, 255, 255, 0.6);
        }

        .bar-submit {
          background: rgba(255, 255, 255, 0.2);
          border: none;
          border-radius: 12px;
          padding: 4px;
          color: white;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .bar-submit:hover:not(:disabled) {
          background: rgba(255, 255, 255, 0.3);
        }

        .bar-submit:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .bar-right {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 24px;
          height: 24px;
        }

        .bar-visualizer {
          width: 100%;
          height: 100%;
        }

        .response-container {
          background: rgba(255, 255, 255, 0.1);
          backdrop-filter: blur(20px) saturate(180%);
          border: 1px solid rgba(255, 255, 255, 0.2);
          border-radius: 16px;
          overflow: hidden;
        }

        .response-header {
          padding: 12px 16px;
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }

        .response-title {
          display: flex;
          align-items: center;
          justify-content: space-between;
          color: white;
          font-weight: 600;
          font-size: 14px;
        }

        .response-actions {
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .response-toggle,
        .response-close {
          background: rgba(255, 255, 255, 0.1);
          border: none;
          border-radius: 8px;
          padding: 4px;
          color: white;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .response-toggle:hover,
        .response-close:hover {
          background: rgba(255, 255, 255, 0.2);
        }

        .response-content {
          overflow: hidden;
          transition: all 0.3s ease;
        }

        .response-content.collapsed {
          max-height: 100px;
        }

        .response-content.expanded {
          max-height: 500px;
        }

        .response-item {
          padding: 16px;
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }

        .response-item:last-child {
          border-bottom: none;
        }

        .text-block,
        .code-block,
        .image-block,
        .video-block {
          color: white;
        }

        .text-header,
        .code-header,
        .image-header,
        .video-header {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 8px;
          font-weight: 600;
          font-size: 14px;
        }

        .text-content {
          font-size: 14px;
          line-height: 1.5;
        }

        .code-content {
          background: rgba(0, 0, 0, 0.3);
          border-radius: 8px;
          padding: 12px;
          font-family: "Monaco", "Menlo", "Consolas", monospace;
          font-size: 12px;
          overflow-x: auto;
        }

        .image-container,
        .video-container {
          border-radius: 8px;
          overflow: hidden;
        }

        .image-container img,
        .video-container video {
          width: 100%;
          height: auto;
          display: block;
        }

        .copy-btn {
          background: rgba(255, 255, 255, 0.1);
          border: none;
          border-radius: 6px;
          padding: 4px;
          color: white;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .copy-btn:hover {
          background: rgba(255, 255, 255, 0.2);
        }

        .bar-actions {
          display: flex;
          align-items: center;
          gap: 8px;
          justify-content: center;
        }

        .action-btn {
          background: rgba(255, 255, 255, 0.1);
          border: none;
          border-radius: 12px;
          padding: 8px;
          color: white;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .action-btn:hover {
          background: rgba(255, 255, 255, 0.2);
        }

        .action-btn.active {
          background: rgba(59, 130, 246, 0.3);
          color: #60a5fa;
        }

        /* State-specific styling */
        .state-listening .bar-main {
          background: rgba(59, 130, 246, 0.2);
          border-color: rgba(59, 130, 246, 0.4);
        }

        .state-processing .bar-main {
          background: rgba(251, 191, 36, 0.2);
          border-color: rgba(251, 191, 36, 0.4);
        }

        .state-speaking .bar-main {
          background: rgba(34, 197, 94, 0.2);
          border-color: rgba(34, 197, 94, 0.4);
        }

        .state-error .bar-main {
          background: rgba(239, 68, 68, 0.2);
          border-color: rgba(239, 68, 68, 0.4);
        }

        .state-success .bar-main {
          background: rgba(34, 197, 94, 0.2);
          border-color: rgba(34, 197, 94, 0.4);
        }

        .state-input .bar-main {
          background: rgba(255, 255, 255, 0.15);
          border-color: rgba(255, 255, 255, 0.3);
        }

        /* Responsive */
        @media (max-width: 768px) {
          .voice-ai-bar {
            left: 10px;
            right: 10px;
            transform: none;
          }

          .bar-main {
            min-width: auto;
          }

          .response-content.expanded {
            max-height: 300px;
          }
        }
      `}</style>
    </div>
  );
}
