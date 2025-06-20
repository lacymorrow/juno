import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettingsManager } from "@/hooks/useSettingsManager";
import { Brain, Zap, Settings, User, Users } from "lucide-react";
import { useState } from "react";

interface MobileAIControlsProps {
  className?: string;
}

export function MobileAIControls({ className = "" }: MobileAIControlsProps) {
  const settingsManager = useSettingsManager();
  const [loading, setLoading] = useState(false);

  const currentProvider =
    settingsManager.providers?.active_provider || "anthropic";
  const currentMode = settingsManager.agent?.mode || "multi";
  const providers = settingsManager.providers?.providers || [];

  const handleProviderChange = async (providerId: string) => {
    setLoading(true);
    try {
      await settingsManager.updateProviders({ active_provider: providerId });
    } catch (error) {
      console.error("Failed to update provider:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleModeChange = async (mode: string) => {
    setLoading(true);
    try {
      await settingsManager.updateAgent({ mode });
    } catch (error) {
      console.error("Failed to update agent mode:", error);
    } finally {
      setLoading(false);
    }
  };

  if (settingsManager.loading) {
    return (
      <div className={`space-y-4 p-4 ${className}`}>
        <div className="text-center text-gray-500">Loading AI settings...</div>
      </div>
    );
  }

  return (
    <div className={`space-y-4 p-4 ${className}`}>
      <div className="space-y-2">
        <label className="text-sm font-medium">AI Provider</label>
        <Select
          value={currentProvider}
          onValueChange={handleProviderChange}
          disabled={loading}
        >
          <SelectTrigger>
            <SelectValue placeholder="Select Provider" />
          </SelectTrigger>
          <SelectContent>
            {providers.map((provider) => (
              <SelectItem key={provider.id} value={provider.id}>
                <div className="flex items-center gap-2">
                  <Brain className="h-4 w-4" />
                  <span>{provider.display_name}</span>
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-2">
        <label className="text-sm font-medium">Agent Mode</label>
        <Select
          value={currentMode}
          onValueChange={handleModeChange}
          disabled={loading}
        >
          <SelectTrigger>
            <SelectValue placeholder="Select Mode" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="single">
              <div className="flex items-center gap-2">
                <User className="h-4 w-4" />
                <span>Single Agent</span>
                <Badge variant="outline" className="text-xs">
                  Simple
                </Badge>
              </div>
            </SelectItem>
            <SelectItem value="multi">
              <div className="flex items-center gap-2">
                <Users className="h-4 w-4" />
                <span>Multi-Agent</span>
                <Badge variant="outline" className="text-xs">
                  Advanced
                </Badge>
              </div>
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="text-xs text-gray-500 space-y-1">
        <div>
          Current Provider:{" "}
          {providers.find((p) => p.id === currentProvider)?.display_name ||
            "Unknown"}
        </div>
        <div>
          Agent Mode: {currentMode === "multi" ? "Multi-Agent" : "Single Agent"}
        </div>
      </div>
    </div>
  );
}
