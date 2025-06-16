import React from "react";
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

interface ModelSelectorProps {
  variant?: "compact" | "full";
  className?: string;
}

export function ModelSelector({
  variant = "compact",
  className = "",
}: ModelSelectorProps) {
  const settings = useSettings();

  const currentProvider = settings.providers.find(
    (p) => p.id === settings.activeProvider
  );

  if (!currentProvider || settings.isLoading) {
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
          <Brain size={14} className="text-muted-foreground" />
          <span className="text-xs text-muted-foreground">Model:</span>
        </div>
      )}

      <Select
        value={settings.formData.model}
        onValueChange={async (value) => {
          settings.setFormData((prev) => ({ ...prev, model: value }));
          // Auto-save when model changes
          if (settings.activeProvider) {
            try {
              await settings.handleSaveProviderSettings();
            } catch (error) {
              console.error("Failed to save model selection:", error);
            }
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
            {(() => {
              if (settings.formData.model && currentProvider?.model_info) {
                const selectedModel = currentProvider.model_info.find(
                  (m) => m.id === settings.formData.model
                );
                return (
                  <div className="flex items-center gap-1">
                    <span className="text-xs">
                      {selectedModel?.supports_computer_use ? "🖥️" : "💬"}
                    </span>
                    <span className="text-xs font-medium">
                      {variant === "compact"
                        ? selectedModel?.name?.replace(/Claude\s*/i, "") ||
                          selectedModel?.name
                        : selectedModel?.name}
                    </span>
                  </div>
                );
              }
              return <span className="text-xs">Select model</span>;
            })()}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {currentProvider?.model_info ? (
            <>
              {/* Computer Use Models */}
              {currentProvider.model_info.filter(
                (model) => model.supports_computer_use
              ).length > 0 && (
                <>
                  <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-blue-50 border-b">
                    <div className="flex items-center gap-1">
                      <Cpu size={12} />
                      Computer Use Models
                    </div>
                  </div>
                  {currentProvider.model_info
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
              {currentProvider.model_info.filter(
                (model) => !model.supports_computer_use
              ).length > 0 && (
                <>
                  <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-gray-50 border-b">
                    <div className="flex items-center gap-1">
                      <Brain size={12} />
                      General Chat Models
                    </div>
                  </div>
                  {currentProvider.model_info
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
            </>
          ) : (
            // Fallback to old model list format
            currentProvider?.models?.map((model) => (
              <SelectItem key={model} value={model}>
                {model}
              </SelectItem>
            )) || (
              <SelectItem value="" disabled>
                No models available
              </SelectItem>
            )
          )}
        </SelectContent>
      </Select>

      {variant === "full" &&
        settings.formData.model &&
        currentProvider?.model_info && (
          <div className="text-xs text-muted-foreground">
            {(() => {
              const selectedModel = currentProvider.model_info.find(
                (m) => m.id === settings.formData.model
              );
              if (selectedModel?.supports_computer_use) {
                return (
                  <Badge
                    variant="outline"
                    className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                  >
                    Computer Use
                  </Badge>
                );
              } else if (selectedModel) {
                return (
                  <Badge
                    variant="outline"
                    className="text-xs bg-gray-50 text-gray-700 border-gray-200"
                  >
                    Chat Only
                  </Badge>
                );
              }
              return null;
            })()}
          </div>
        )}
    </div>
  );
}
