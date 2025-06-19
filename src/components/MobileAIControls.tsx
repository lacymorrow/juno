import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useSettings } from "@/hooks/useSettings";
import { ChevronDown } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MobileAIControlsProps {
  className?: string;
}

interface ModelInfo {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
}

export function MobileAIControls({ className = "" }: MobileAIControlsProps) {
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

  const selectedModel = availableModels.find(
    (m) => m.id === settings.formData.model
  );

  if (!currentProvider || loadingModels) {
    return (
      <div className={`flex flex-col gap-2 ${className}`}>
        <div className="w-full h-8 bg-muted animate-pulse rounded" />
        <div className="w-full h-8 bg-muted animate-pulse rounded" />
      </div>
    );
  }

  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {/* Provider Selector */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            className="w-full justify-between text-xs"
            size="sm"
          >
            <div className="flex items-center gap-2">
              <span className="font-medium">{currentProvider.name}</span>
              {currentProvider.computer_use_supported && (
                <Badge
                  variant="outline"
                  className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                >
                  CU
                </Badge>
              )}
            </div>
            <ChevronDown className="h-3 w-3" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-56">
          {settings.providers.map((provider) => (
            <DropdownMenuItem
              key={provider.id}
              onClick={() => settings.handleActiveProviderChange(provider.id)}
              className="text-xs"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium">{provider.name}</span>
                {provider.computer_use_supported && (
                  <Badge
                    variant="outline"
                    className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                  >
                    Computer Use
                  </Badge>
                )}
              </div>
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      {/* Model Selector */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            className="w-full justify-between text-xs"
            size="sm"
          >
            <div className="flex items-center gap-2">
              {selectedModel ? (
                <>
                  <span>
                    {selectedModel.supports_computer_use ? "🖥️" : "💬"}
                  </span>
                  <span className="font-medium">
                    {selectedModel.name.length > 20
                      ? `${selectedModel.name.substring(0, 17)}...`
                      : selectedModel.name}
                  </span>
                </>
              ) : (
                <span className="text-muted-foreground">Select model</span>
              )}
            </div>
            <ChevronDown className="h-3 w-3" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-56">
          {availableModels.length > 0 ? (
            <>
              {availableModels
                .filter((model) => model.id !== settings.formData.model)
                .slice(0, 4) // Show max 4 for mobile
                .map((model) => (
                  <DropdownMenuItem
                    key={model.id}
                    onClick={async () => {
                      // Validate model before setting
                      try {
                        const isValid = await invoke<boolean>(
                          "validate_provider_model",
                          {
                            providerId: settings.activeProvider,
                            modelId: model.id,
                          }
                        );

                        if (isValid) {
                          settings.setFormData((prev) => ({
                            ...prev,
                            model: model.id,
                          }));
                          if (settings.activeProvider) {
                            try {
                              await settings.handleSaveProviderSettings();
                            } catch (error) {
                              console.error(
                                "Failed to save model selection:",
                                error
                              );
                            }
                          }
                        } else {
                          console.warn(
                            `Model ${model.id} is not valid for provider ${settings.activeProvider}`
                          );
                        }
                      } catch (error) {
                        console.error("Failed to validate model:", error);
                      }
                    }}
                    className="text-xs"
                  >
                    <div className="flex items-center gap-2">
                      <span>{model.supports_computer_use ? "🖥️" : "💬"}</span>
                      <div className="flex flex-col">
                        <span className="font-medium">{model.name}</span>
                        {model.is_recommended && (
                          <Badge
                            variant="outline"
                            className="text-xs bg-green-50 text-green-700 border-green-200 w-fit"
                          >
                            Recommended
                          </Badge>
                        )}
                      </div>
                    </div>
                  </DropdownMenuItem>
                ))}
            </>
          ) : (
            <DropdownMenuItem disabled className="text-xs">
              No models available
            </DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
