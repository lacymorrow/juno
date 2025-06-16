import { AgentExecutionProgressIndicator } from "@/components/AgentExecutionProgressIndicator";
import { Button } from "@/components/ui/button";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import { ModelSelector } from "@/components/ModelSelector";
import { AgentModeSelector } from "@/components/AgentModeSelector";
import { ProviderSelector } from "@/components/ProviderSelector";
import { cn } from "@/lib/utils";
import {
  ArrowLeft,
  DogIcon,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";

// Type for view state
export type AppView = "chat" | "devtools" | "permissions";

interface AppHeaderProps {
  serverStatus: "connected" | "error" | "connecting";
  isProcessing: boolean;
  currentView: AppView;
  isDevPanelOpen: boolean;
  onViewChange: (view: AppView) => void;
  onToggleDevPanel: () => void;
}

export function AppHeader({
  serverStatus,
  isProcessing,
  currentView,
  isDevPanelOpen,
  onViewChange,
  onToggleDevPanel,
}: AppHeaderProps) {
  return (
    <header className="flex items-center justify-between py-1 px-2 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 min-h-[40px]">
      <div className="flex items-center gap-2 min-w-0 flex-1 max-w-fit">
        <div className="flex items-center gap-1 flex-shrink-0">
          <DogIcon size={16} className="text-blue-500" />
          <span className="text-sm font-semibold">Juno AI</span>
          <div className="flex items-center gap-1">
            <div
              className={cn(
                "w-1.5 h-1.5 rounded-full",
                serverStatus === "connected"
                  ? "bg-green-500"
                  : serverStatus === "error"
                  ? "bg-red-500"
                  : "bg-yellow-500"
              )}
            />
            {isProcessing && (
              <div className="text-xs text-muted-foreground">
                <AgentExecutionProgressIndicator
                  compact
                  className="text-muted-foreground"
                />
              </div>
            )}
          </div>
        </div>

        {/* AI Configuration Selectors - only show in chat view */}
        {currentView === "chat" && (
          <div className="hidden md:flex items-center gap-1 ml-2 border-l border-border pl-2 flex-shrink-0">
            <ProviderSelector variant="compact" />
            <div className="w-px h-4 bg-border" />
            <ModelSelector variant="compact" />
            <div className="w-px h-4 bg-border" />
            <AgentModeSelector variant="compact" />
          </div>
        )}
      </div>

      {/* Voice Status Indicator - only show in chat view */}
      {currentView === "chat" && (
        <div className="flex-1 flex justify-center mx-2 min-w-0">
          <VoiceStatusIndicator
            variant="compact"
            className="max-w-xs truncate"
            showText={false}
          />
        </div>
      )}

      <div className="flex items-center gap-1 flex-shrink-0">
        {/* Back Button - show for devtools, permissions views */}
        {(currentView === "devtools" || currentView === "permissions") && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => onViewChange("chat")}
            title="Back to Chat"
            className="h-7 w-7 p-0"
          >
            <ArrowLeft size={14} />
          </Button>
        )}
        {/* Toggle Dev Panel Button - only show in chat view */}
        {currentView === "chat" && (
          <Button
            variant="outline"
            size="sm"
            onClick={onToggleDevPanel}
            title={isDevPanelOpen ? "Hide Dev Panel" : "Show Dev Panel"}
            className="h-7 w-7 p-0"
          >
            {isDevPanelOpen ? (
              <PanelLeftClose size={14} />
            ) : (
              <PanelLeftOpen size={14} />
            )}
          </Button>
        )}
      </div>
    </header>
  );
}
