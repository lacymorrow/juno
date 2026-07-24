import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useDragWindow } from "@/hooks/useDragWindow";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { AgentSessionRows } from "@/components/AgentSessionRows";
import { UI } from "@/lib/constants.generated";
import { ChevronUp, ChevronDown, X } from "lucide-react";

interface FloatingPanelProps {
  isVisible: boolean;
  agentStatus:
    | typeof UI.AGENT_STATUS_IDLE
    | typeof UI.AGENT_STATUS_LISTENING
    | typeof UI.AGENT_STATUS_THINKING
    | typeof UI.AGENT_STATUS_RESPONDING
    | typeof UI.AGENT_STATUS_ERROR;
  message?: string;
  onModeChange?: (mode: "compact" | "expanded" | "chat" | "settings") => void;
}

const TransparentFloatingPanel: React.FC<FloatingPanelProps> = ({
  isVisible,
  agentStatus = UI.AGENT_STATUS_IDLE,
  message,
  onModeChange,
}) => {
  const [mode, setMode] = useState<
    "compact" | "expanded" | "chat" | "settings"
  >("compact");
  const [opacity, setOpacity] = useState(0.8);
  const [isHovered, setIsHovered] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const onDragMouseDown = useDragWindow();
  // Parallel agent sessions (LAC-1432) — summary rows in expanded mode
  const { sessions: agentSessions, focusSession } = useAgentSessions();

  // Handle panel visibility
  useEffect(() => {
    if (isVisible) {
      setMode("compact");
    }
  }, [isVisible]);

  // Get dynamic dimensions
  const getDynamicDimensions = () => {
    switch (mode) {
      case "compact":
        return { width: 300, height: 100 };
      case "expanded":
        // Grow with the agent list: 32px per row + header/footer, capped
        // so the list scrolls instead of pushing the panel off screen
        // (LAC-2830 §4).
        return {
          width: 350,
          height:
            250 +
            (agentSessions.length > 0
              ? Math.min(agentSessions.length * 32 + 60, 160)
              : 0),
        };
      case "chat":
        return { width: 280, height: 180 };
      case "settings":
        return { width: 320, height: 200 };
      default:
        return { width: 300, height: 100 };
    }
  };

  // Handle mode changes
  const handleModeChange = (
    newMode: "compact" | "expanded" | "chat" | "settings"
  ) => {
    setMode(newMode);
    onModeChange?.(newMode);
  };

  // Handle panel interaction
  const handlePanelInteraction = async (action: string) => {
    try {
      await invoke("ui_handle_interaction", {
        elementId: "floating-panel",
        interaction: {
          interaction_type: action,
          data: { mode, status: agentStatus },
        },
      });
    } catch (error) {
      console.error("Panel interaction failed:", error);
    }
  };

  if (!isVisible) {
    return null;
  }

  const dimensions = getDynamicDimensions();

  return (
    <div
      ref={panelRef}
      className={`
        fixed top-4 right-4 z-50 rounded-lg border backdrop-blur-sm
        transition-all duration-300 ease-out cursor-grab active:cursor-grabbing
        ${isHovered ? "shadow-lg" : "shadow-md"}
      `}
      onMouseDown={onDragMouseDown}
      style={{
        width: dimensions.width,
        height: dimensions.height,
        backgroundColor: `rgba(255, 255, 255, ${opacity})`,
        borderColor: "rgba(255, 255, 255, 0.3)",
        transform: isHovered ? "translateZ(0px)" : "translateZ(-20px)",
      }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* Panel Header */}
      <div className="flex items-center justify-between p-3 border-b border-white/20">
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${
              agentStatus === UI.AGENT_STATUS_LISTENING
                ? "bg-blue-500"
                : agentStatus === UI.AGENT_STATUS_THINKING
                ? "bg-yellow-500"
                : agentStatus === UI.AGENT_STATUS_RESPONDING
                ? "bg-green-500"
                : agentStatus === UI.AGENT_STATUS_ERROR
                ? "bg-red-500"
                : "bg-gray-400"
            }`}
          />
          <span className="text-sm font-medium">
            {agentStatus === UI.AGENT_STATUS_LISTENING
              ? "Listening"
              : agentStatus === UI.AGENT_STATUS_THINKING
              ? "Thinking"
              : agentStatus === UI.AGENT_STATUS_RESPONDING
              ? "Responding"
              : agentStatus === UI.AGENT_STATUS_ERROR
              ? "Error"
              : "Idle"}
          </span>
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={() =>
              handleModeChange(mode === "compact" ? "expanded" : "compact")
            }
            className="p-1 rounded hover:bg-white/20 transition-colors"
          >
            {mode === "compact" ? (
              <ChevronUp size={14} />
            ) : (
              <ChevronDown size={14} />
            )}
          </button>

          <button
            onClick={() => handlePanelInteraction("close")}
            className="p-1 rounded hover:bg-white/20 transition-colors"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Panel Content */}
      <div className="p-3">
        {mode === "compact" && (
          <div className="text-sm text-gray-700">
            {message || "Agent is ready"}
          </div>
        )}

        {mode === "expanded" && (
          <div className="space-y-2">
            <div className="text-sm text-gray-700">
              {message || "Agent is ready"}
            </div>

            <AgentSessionRows sessions={agentSessions} onFocus={focusSession} />

            <div className="flex items-center gap-2">
              <label className="text-xs text-gray-600">Opacity:</label>
              <input
                type="range"
                min="0.3"
                max="1"
                step="0.1"
                value={opacity}
                onChange={(e) => setOpacity(parseFloat(e.target.value))}
                className="flex-1"
              />
            </div>

            <div className="flex gap-2">
              <button
                onClick={() => handleModeChange("chat")}
                className="px-2 py-1 text-xs bg-blue-100 rounded hover:bg-blue-200 transition-colors"
              >
                Chat
              </button>
              <button
                onClick={() => handleModeChange("settings")}
                className="px-2 py-1 text-xs bg-gray-100 rounded hover:bg-gray-200 transition-colors"
              >
                Settings
              </button>
            </div>
          </div>
        )}

        {mode === "chat" && (
          <div className="text-sm text-gray-700">Chat mode - Coming soon</div>
        )}

        {mode === "settings" && (
          <div className="space-y-2">
            <div className="text-sm font-medium">Panel Settings</div>
            <div className="text-xs text-gray-600">
              Customize panel behavior
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default TransparentFloatingPanel;
