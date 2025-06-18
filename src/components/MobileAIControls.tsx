import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Badge } from "@/components/ui/badge";
import { useSettings } from "@/hooks/useSettings";
import { ChevronDown, Brain, Settings2, Users, User } from "lucide-react";

interface MobileAIControlsProps {
  className?: string;
}

export function MobileAIControls({ className = "" }: MobileAIControlsProps) {
  const settings = useSettings();
  const [isOpen, setIsOpen] = useState(false);

  const currentProvider = settings.providers.find(
    (p) => p.id === settings.activeProvider
  );

  const currentModel = currentProvider?.model_info?.find(
    (m) => m.id === settings.formData.model
  );

  const currentMode = settings.agentMode === "multi" ? "Multi" : "Single";

  if (settings.isLoading) {
    return (
      <Button variant="ghost" size="sm" disabled className={`h-7 ${className}`}>
        <div className="w-4 h-4 bg-muted animate-pulse rounded" />
      </Button>
    );
  }

  return (
    <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          className={`h-7 px-2 text-xs border-none bg-transparent hover:bg-muted/50 ${className}`}
        >
          <div className="flex items-center gap-1">
            <Settings2 size={12} />
            <span className="max-w-16 truncate">
              {currentModel?.supports_computer_use ? "🖥️" : "💬"} {currentMode}
            </span>
            <ChevronDown size={10} />
          </div>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel className="flex items-center gap-2">
          <Brain size={14} />
          AI Configuration
        </DropdownMenuLabel>
        <DropdownMenuSeparator />

        {/* Current Status */}
        <div className="px-2 py-1 text-xs">
          <div className="flex items-center justify-between mb-1">
            <span className="text-muted-foreground">Provider:</span>
            <span className="font-medium">
              {currentProvider?.name || "None"}
            </span>
          </div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-muted-foreground">Model:</span>
            <div className="flex items-center gap-1">
              <span className="text-xs">
                {currentModel?.supports_computer_use ? "🖥️" : "💬"}
              </span>
              <span className="font-medium text-xs max-w-24 truncate">
                {currentModel?.name || "None"}
              </span>
            </div>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground">Mode:</span>
            <div className="flex items-center gap-1">
              <span className="text-xs">
                {settings.agentMode === "multi" ? "👥" : "🤖"}
              </span>
              <span className="font-medium">{currentMode} Agent</span>
            </div>
          </div>
        </div>

        <DropdownMenuSeparator />

        {/* Quick Actions */}
        <DropdownMenuItem
          onClick={() =>
            settings.handleAgentModeChange(
              settings.agentMode === "multi" ? "single" : "multi"
            )
          }
          className="text-xs"
        >
          <div className="flex items-center gap-2">
            {settings.agentMode === "multi" ? (
              <User size={12} />
            ) : (
              <Users size={12} />
            )}
            Switch to {settings.agentMode === "multi" ? "Single" : "Multi"}{" "}
            Agent
          </div>
        </DropdownMenuItem>

        {currentProvider?.model_info &&
          currentProvider.model_info.length > 1 && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuLabel className="text-xs">
                Quick Model Switch
              </DropdownMenuLabel>
              {currentProvider.model_info
                .filter((model) => model.id !== settings.formData.model)
                .slice(0, 3) // Show max 3 for mobile
                .map((model) => (
                  <DropdownMenuItem
                    key={model.id}
                    onClick={async () => {
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
          )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
