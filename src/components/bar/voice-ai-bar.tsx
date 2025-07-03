"use client";

import React, { useState, useRef } from "react";
import {
  Mic,
  Send,
  Zap,
  AlertCircle,
  CheckCircle,
  Volume2,
  Code,
  ImageIcon,
} from "lucide-react";

import AudioVisualizer from "./audio-visualizer";
import {
  VoiceAIBarProps,
  AssistantState,
  ResponseContent,
} from "../../types/voice-ai";
import { UI } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

export function VoiceAIBar({
  onStateChange,
  initialState = UI.AGENT_STATUS_IDLE,
  className = "",
}: VoiceAIBarProps) {
  const [assistantState, setAssistantState] =
    useState<AssistantState>(initialState);
  const [responseContent] = useState<ResponseContent[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [isVisible, setIsVisible] = useState(false);
  const [dimensions] = useState({ width: 400, height: 60 });
  const [barRef] = useState<React.RefObject<HTMLDivElement>>(
    useRef<HTMLDivElement>(null)
  );

  // State message map
  const stateMessages: Record<AssistantState, string> = {
    [UI.AGENT_STATUS_IDLE]: "Idle",
    [UI.AGENT_STATUS_LISTENING]: "Listening...",
    [UI.AGENT_STATUS_PROCESSING]: "Processing...",
    [UI.AGENT_STATUS_RESPONDING]: "Responding...",
    [UI.AGENT_STATUS_ERROR]: "Error",
    [UI.AGENT_STATUS_FINISHED]: "Finished",
  };

  // Handle state change
  const changeState = (newState: AssistantState) => {
    setAssistantState(newState);
    onStateChange?.(newState);

    // Handle state-specific actions
    if (newState === UI.AGENT_STATUS_FINISHED) {
      setIsVisible(false);
    }

    if (newState === UI.AGENT_STATUS_ERROR) {
      setTimeout(() => {
        changeState(UI.AGENT_STATUS_FINISHED);
      }, 3000);
    }
  };

  // Handle input submission
  const handleInputSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputValue.trim()) return;

    // Process the input
    changeState(UI.AGENT_STATUS_PROCESSING);
    setInputValue("");

    // Simulate response
    setTimeout(() => {
      changeState(UI.AGENT_STATUS_RESPONDING);
    }, 1000);
  };

  // Get bar styling class
  const getBarClass = () => {
    const baseClass = "voice-ai-bar";
    switch (assistantState) {
      case UI.AGENT_STATUS_LISTENING:
        return `${baseClass} listening`;
      case UI.AGENT_STATUS_PROCESSING:
        return `${baseClass} processing`;
      case UI.AGENT_STATUS_RESPONDING:
        return `${baseClass} responding`;
      case UI.AGENT_STATUS_ERROR:
        return `${baseClass} error`;
      default:
        return baseClass;
    }
  };

  // Get dynamic styling
  const getDynamicStyle = () => {
    return {
      width: dimensions.width,
      height: dimensions.height,
      borderRadius: "12px",
      backgroundColor: "rgba(255, 255, 255, 0.95)",
      backdropFilter: "blur(20px)",
      boxShadow: "0 8px 32px rgba(0, 0, 0, 0.12)",
    };
  };

  if (!isVisible) {
    return null;
  }

  return (
    <div
      ref={barRef}
      className={cn(
        getBarClass(),
        className,
        "fixed bottom-4 left-1/2 transform -translate-x-1/2 z-50",
        "transition-all duration-300 ease-out",
        isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
      )}
      style={getDynamicStyle()}
    >
      <div className="glass-backdrop" />

      {/* Input Mode */}
      {assistantState === UI.AGENT_STATUS_FINISHED && (
        <div className="input-container">
          <form onSubmit={handleInputSubmit} className="input-form">
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder="Type your message..."
              className="input-field"
              autoFocus
            />
            <button
              type="submit"
              className="submit-button"
              disabled={!inputValue.trim()}
            >
              <Send size={16} />
            </button>
          </form>
        </div>
      )}

      {/* Audio Visualizer */}
      {assistantState === UI.AGENT_STATUS_LISTENING && (
        <AudioVisualizer
          appState={assistantState}
          className="audio-visualizer"
        />
      )}

      {/* Status Display */}
      <div className="status-display">
        <div className="status-icon">
          {assistantState === UI.AGENT_STATUS_LISTENING && <Mic size={20} />}
          {assistantState === UI.AGENT_STATUS_PROCESSING && <Zap size={20} />}
          {assistantState === UI.AGENT_STATUS_RESPONDING && (
            <Volume2 size={20} />
          )}
          {assistantState === UI.AGENT_STATUS_ERROR && (
            <AlertCircle size={20} />
          )}
          {assistantState === UI.AGENT_STATUS_FINISHED && (
            <CheckCircle size={20} />
          )}
        </div>

        <div className="status-text">{stateMessages[assistantState]}</div>
      </div>

      {/* Response Content */}
      {responseContent.length > 0 && (
        <div className="response-content">
          {responseContent.map((content, index) => (
            <div key={index} className="response-item">
              {content.type === "text" && (
                <div className="text-content">{content.content}</div>
              )}
              {content.type === "code" && (
                <div className="code-content">
                  <Code size={16} />
                  <pre>{content.content}</pre>
                </div>
              )}
              {content.type === "image" && (
                <div className="image-content">
                  <ImageIcon size={16} />
                  <span>{content.content}</span>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
