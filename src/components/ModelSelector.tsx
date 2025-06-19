import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettings } from "@/hooks/useSettings";
import { Brain, Cpu } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ModelSelectorProps {
  variant?: "compact" | "full";
  className?: string;
}

interface ModelInfo {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
}

export function ModelSelector({
  variant = "compact",
  className = "",
}: ModelSelectorProps) {
  const settings = useSettings();
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);

  // Load models when active provider changes
  useEffect(() => {
    if (settings.activeProvider && !settings.isLoading) {
      loadModelsForProvider(settings.activeProvider);
    }
  }, [settings.activeProvider, settings.isLoading]);

  const loadModelsForProvider = async (providerId: string) => {
    setLoadingModels(true);
    try {
      const models = await invoke<ModelInfo[]>("get_provider_models", {
        providerId,
      });
      setAvailableModels(models);
    } catch (error) {
      console.error("Failed to load models for provider:", error);
      setAvailableModels([]);
    } finally {
      setLoadingModels(false);
    }
  };

  const currentProvider = settings.providers.find(
    (p) => p.id === settings.activeProvider
  );

  if (!currentProvider || settings.isLoading || loadingModels) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="w-4 h-4 bg-muted animate-pulse rounded" />
        <div className="w-20 h-4 bg-muted animate-pulse rounded" />
      </div>
    );
  }

  // If no models are available, show an error state
  if (availableModels.length === 0) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        {variant === "full" && (
          <div className="flex items-center gap-1">
            <Brain size={14} className="text-muted-foreground" />
            <span className="text-xs text-muted-foreground">Model:</span>
          </div>
        )}
        <span className="text-xs text-red-500">No models available</span>
      </div>
    );
  }

  const selectedModel = availableModels.find(
    (m) => m.id === settings.formData.model
  );

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {variant === "full" && (
        <div className="flex items-center gap-1">
          <Brain size={14} className="text-muted-foreground" />
          <span className="text-xs text-muted-foreground">Model:</span>
        </div>
      )}

      <Select
        value={settings.formData.model}
        onValueChange={async (value) => {
          // Validate model is available for current provider before setting
          try {
            const isValid = await invoke<boolean>("validate_provider_model", {
              providerId: settings.activeProvider,
              modelId: value,
            });

            if (isValid) {
              settings.setFormData((prev) => ({ ...prev, model: value }));
              // Auto-save when model changes
              if (settings.activeProvider) {
                try {
                  await settings.handleSaveProviderSettings();
                } catch (error) {
                  console.error("Failed to save model selection:", error);
                }
              }
            } else {
              console.warn(
                `Model ${value} is not valid for provider ${settings.activeProvider}`
              );
            }
          } catch (error) {
            console.error("Failed to validate model:", error);
          }
        }}
      >
        <SelectTrigger
          className={
            variant === "compact"
              ? "h-7 text-xs border-none bg-transparent hover:bg-muted/50 w-auto"
              : "h-8"
          }
        >
          <SelectValue placeholder="Select model">
            {selectedModel ? (
              <div className="flex items-center gap-1">
                <span className="text-xs">
                  {selectedModel.supports_computer_use ? "🖥️" : "💬"}
                </span>
                <span className="text-xs font-medium">
                  {variant === "compact"
                    ? selectedModel.name.replace(/Claude\s*/i, "")
                    : selectedModel.name}
                </span>
              </div>
            ) : (
              <span className="text-xs">Select model</span>
            )}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {/* Computer Use Models */}
          {availableModels.filter((model) => model.supports_computer_use)
            .length > 0 && (
            <>
              <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-blue-50 border-b">
                <div className="flex items-center gap-1">
                  <Cpu size={12} />
                  Computer Use Models
                </div>
              </div>
              {availableModels
                .filter((model) => model.supports_computer_use)
                .map((model) => (
                  <SelectItem key={model.id} value={model.id}>
                    <div className="flex items-center gap-2">
                      <span>🖥️</span>
                      <span>{model.name}</span>
                      {model.is_recommended && (
                        <Badge
                          variant="outline"
                          className="text-xs bg-green-50 text-green-700 border-green-200"
                        >
                          Recommended
                        </Badge>
                      )}
                    </div>
                  </SelectItem>
                ))}
            </>
          )}

          {/* General Chat Models */}
          {availableModels.filter((model) => !model.supports_computer_use)
            .length > 0 && (
            <>
              <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-gray-50 border-b">
                <div className="flex items-center gap-1">
                  <Brain size={12} />
                  General Chat Models
                </div>
              </div>
              {availableModels
                .filter((model) => !model.supports_computer_use)
                .map((model) => (
                  <SelectItem key={model.id} value={model.id}>
                    <div className="flex items-center gap-2">
                      <span>💬</span>
                      <span>{model.name}</span>
                      {model.is_recommended && (
                        <Badge
                          variant="outline"
                          className="text-xs bg-green-50 text-green-700 border-green-200"
                        >
                          Recommended
                        </Badge>
                      )}
                    </div>
                  </SelectItem>
                ))}
            </>
          )}
        </SelectContent>
      </Select>

      {variant === "full" && selectedModel && (
        <div className="text-xs text-muted-foreground">
          {selectedModel.supports_computer_use ? (
            <Badge
              variant="outline"
              className="text-xs bg-blue-50 text-blue-700 border-blue-200"
            >
              Computer Use
            </Badge>
          ) : (
            <Badge
              variant="outline"
              className="text-xs bg-gray-50 text-gray-700 border-gray-200"
            >
              Chat Only
            </Badge>
          )}
        </div>
      )}
    </div>
  );
}
