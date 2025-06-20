import React from "react";
import { useSettingsManager } from "@/hooks/useSettingsManager";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Settings, Users, User, Bot } from "lucide-react";

interface AgentModeSelectorProps {
  variant?: "compact" | "full";
  disabled?: boolean;
  className?: string;
}

const agentModes = [
  {
    id: "single",
    name: "Single Agent",
    description: "One AI assistant handles all tasks",
    icon: <User className="h-4 w-4" />,
    badge: "Simple",
  },
  {
    id: "multi",
    name: "Multi-Agent",
    description: "Specialized agents collaborate on complex tasks",
    icon: <Users className="h-4 w-4" />,
    badge: "Advanced",
  },
];

export function AgentModeSelector({
  variant = "compact",
  disabled = false,
  className = "",
}: AgentModeSelectorProps) {
  const settingsManager = useSettingsManager();

  const handleModeChange = async (mode: string) => {
    try {
      await settingsManager.updateAgent({ mode });
    } catch (error) {
      console.error("Failed to update agent mode:", error);
    }
  };

  const currentMode = settingsManager.agent?.mode || "multi";

  if (settingsManager.loading) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="h-4 w-4 animate-spin rounded-full border-2 border-blue-600 border-t-transparent" />
        <span className="text-sm text-gray-600">Loading agent mode...</span>
      </div>
    );
  }

  return (
    <div className={`space-y-2 ${className}`}>
      <Select
        value={currentMode}
        onValueChange={handleModeChange}
        disabled={disabled}
      >
        <SelectTrigger className="min-w-[180px]">
          <SelectValue placeholder="Select Agent Mode" />
        </SelectTrigger>
        <SelectContent>
          {agentModes.map((mode) => (
            <SelectItem key={mode.id} value={mode.id}>
              <div className="flex items-center gap-2">
                {mode.icon}
                <span>{mode.name}</span>
                <Badge variant="outline" className="text-xs">
                  {mode.badge}
                </Badge>
              </div>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {variant === "full" && (
        <div className="text-xs text-gray-500">
          {agentModes.find((m) => m.id === currentMode)?.description}
        </div>
      )}
    </div>
  );
}
