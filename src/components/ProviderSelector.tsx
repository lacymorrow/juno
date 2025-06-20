import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettingsManager } from "@/hooks/useSettingsManager";
import { Brain, Zap, Check } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ProviderSelectorProps {
  variant?: "compact" | "full";
  disabled?: boolean;
  className?: string;
}

export function ProviderSelector({
  variant = "compact",
  disabled = false,
  className = "",
}: ProviderSelectorProps) {
  const settingsManager = useSettingsManager();
  const [loading, setLoading] = useState(false);

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

  const currentProvider =
    settingsManager.providers?.active_provider || "anthropic";
  const providers = settingsManager.providers?.providers || [];

  if (settingsManager.loading) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="h-4 w-4 animate-spin rounded-full border-2 border-blue-600 border-t-transparent" />
        <span className="text-sm text-gray-600">Loading providers...</span>
      </div>
    );
  }

  return (
    <div className={`space-y-2 ${className}`}>
      <Select
        value={currentProvider}
        onValueChange={handleProviderChange}
        disabled={disabled || loading}
      >
        <SelectTrigger className="min-w-[180px]">
          <SelectValue placeholder="Select AI Provider" />
        </SelectTrigger>
        <SelectContent>
          {providers.map((provider) => (
            <SelectItem key={provider.id} value={provider.id}>
              <div className="flex items-center gap-2">
                <Brain className="h-4 w-4" />
                <span>{provider.display_name}</span>
                {provider.id === currentProvider && (
                  <Check className="h-4 w-4 text-green-600" />
                )}
              </div>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {variant === "full" && (
        <div className="text-xs text-gray-500">
          Current:{" "}
          {providers.find((p) => p.id === currentProvider)?.display_name ||
            "Unknown"}
        </div>
      )}
    </div>
  );
}
