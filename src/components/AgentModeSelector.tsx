import React, { useCallback } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useSettings } from "@/hooks/useSettings";
import { Users, User, Zap } from "lucide-react";

interface AgentModeSelectorProps {
  variant?: "compact" | "full";
  className?: string;
}

const AGENT_MODES = [
  {
    id: "single",
    name: "Single",
    fullName: "Single Agent",
    description: "One AI model handles all tasks",
    icon: <User size={12} />,
    emoji: "🤖",
    dotColor: "bg-green-500",
  },
  {
    id: "multi",
    name: "Multiple",
    fullName: "Multi-Agent",
    description: "Specialized agents for different tasks",
    icon: <Users size={12} />,
    emoji: "👥",
    dotColor: "bg-blue-500",
  },
];

export const AgentModeSelector = React.memo(function AgentModeSelector({
  variant = "compact",
  className = "",
}: AgentModeSelectorProps) {
  const settings = useSettings();

  const currentMode = AGENT_MODES.find(
    (mode) => mode.id === settings.agentMode
  );

  const handleToggle = useCallback(async () => {
    const newMode = settings.agentMode === "single" ? "multi" : "single";
    try {
      await settings.handleAgentModeChange(newMode);
    } catch (error) {
      console.error("Failed to change agent mode:", error);
      // The error handling is already done in the hook, but we can add additional UI feedback here if needed
    }
  }, [settings.agentMode, settings.handleAgentModeChange]);

  if (settings.isLoading) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="w-4 h-4 bg-muted animate-pulse rounded" />
        <div className="w-20 h-4 bg-muted animate-pulse rounded" />
      </div>
    );
  }

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {variant === "full" && (
        <div className="flex items-center gap-1">
          <Zap size={14} className="text-muted-foreground" />
          <span className="text-xs text-muted-foreground">Mode:</span>
        </div>
      )}

      <Button
        variant="ghost"
        size="sm"
        onClick={handleToggle}
        className={
          variant === "compact"
            ? "h-7 text-xs border-none bg-transparent hover:bg-muted/50 px-2 gap-2"
            : "h-8 px-3 gap-2"
        }
        title={`Switch to ${
          settings.agentMode === "single" ? "Multi" : "Single"
        } Agent Mode`}
      >
        <div
          className={`w-2 h-2 rounded-full ${
            currentMode?.dotColor || "bg-gray-400"
          }`}
        />
        <span className="text-xs font-medium">
          {currentMode?.name || "Single"}
        </span>
      </Button>

      {variant === "full" && currentMode && (
        <div className="text-xs text-muted-foreground">
          <Badge
            variant="outline"
            className={`text-xs ${
              currentMode.id === "multi"
                ? "bg-blue-50 text-blue-700 border-blue-200"
                : "bg-gray-50 text-gray-700 border-gray-200"
            }`}
          >
            {currentMode.description}
          </Badge>
        </div>
      )}
    </div>
  );
});
