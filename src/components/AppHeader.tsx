import { AgentExecutionProgressIndicator } from "@/components/AgentExecutionProgressIndicator";
import { Button } from "@/components/ui/button";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import { ModelSelector } from "@/components/ModelSelector";
import { AgentModeSelector } from "@/components/AgentModeSelector";
import { ProviderSelector } from "@/components/ProviderSelector";
import { useSettings } from "@/hooks/useSettings";
import { cn } from "@/lib/utils";
import {
  ArrowLeft,
  DogIcon,
  PanelLeftClose,
  PanelLeftOpen,
  Bug,
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
  const settings = useSettings();

  return (
    <header className="flex items-center py-3 px-4 border-b border-border/50 bg-background/80 backdrop-blur-xl min-h-[56px] shadow-sm">
      <div className="flex items-center gap-3 flex-shrink-0">
        {/* App Identity Section */}
        <div className="flex items-center gap-2 flex-shrink-0">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-gradient-to-r from-blue-50 to-indigo-50 dark:from-blue-950/50 dark:to-indigo-950/50 border border-blue-200/50 dark:border-blue-800/50">
            <DogIcon size={18} className="text-blue-600 dark:text-blue-400" />
            <span className="text-sm font-semibold bg-gradient-to-r from-blue-700 to-indigo-700 dark:from-blue-300 dark:to-indigo-300 bg-clip-text text-transparent">
              Juno AI
            </span>

            {/* Enhanced Status Indicator */}
            <div className="flex items-center gap-2">
              <div
                className={cn(
                  "w-2 h-2 rounded-full transition-colors duration-300",
                  serverStatus === "connected"
                    ? "bg-green-500 shadow-sm shadow-green-500/50"
                    : serverStatus === "error"
                    ? "bg-red-500 shadow-sm shadow-red-500/50 animate-pulse"
                    : "bg-yellow-500 shadow-sm shadow-yellow-500/50 animate-pulse"
                )}
              />
              {isProcessing && (
                <div className="text-xs text-muted-foreground font-medium">
                  <AgentExecutionProgressIndicator
                    compact
                    className="text-blue-600 dark:text-blue-400"
                  />
                </div>
              )}
            </div>
          </div>
        </div>

        {/* AI Configuration Selectors - Enhanced with glass morphism */}
        {currentView === "chat" && (
          <div className="flex items-center gap-2 ml-1 pl-3 border-l border-border/30 flex-shrink-0">
            <div className="flex items-center gap-1 p-1 rounded-lg bg-muted/30 backdrop-blur-sm border border-border/30">
              <ProviderSelector variant="compact" />
              <div className="w-px h-5 bg-border/50" />
              <ModelSelector variant="compact" />
              <div className="w-px h-5 bg-border/50" />
              <AgentModeSelector variant="compact" />
            </div>
          </div>
        )}
      </div>

      {/* Voice Status Indicator - Enhanced center positioning */}
      {currentView === "chat" && (
        <div className="flex items-center justify-center mx-4 min-w-0 flex-1">
          <div className="max-w-md w-full">
            <VoiceStatusIndicator
              variant="compact"
              className="flex-shrink-0 justify-center"
              showText={false}
            />
          </div>
        </div>
      )}

      {/* Action Buttons - Enhanced with better styling */}
      <div className="flex items-center gap-2 flex-shrink-0">
        {/* Debug Button - Enhanced for development */}
        {currentView === "chat" && process.env.NODE_ENV === "development" && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              settings.debugSettings();
              console.log("Manual settings reload triggered");
              settings.loadAllSettings();
            }}
            title="Debug Settings (Dev Only)"
            className="h-8 w-8 p-0 rounded-lg border-orange-200 bg-orange-50 hover:bg-orange-100 text-orange-600 hover:text-orange-700 dark:border-orange-800 dark:bg-orange-950/50 dark:hover:bg-orange-950 dark:text-orange-400"
          >
            <Bug size={14} />
          </Button>
        )}

        {/* Back Button - Enhanced styling */}
        {(currentView === "devtools" || currentView === "permissions") && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => onViewChange("chat")}
            title="Back to Chat"
            className="h-8 px-3 rounded-lg border-blue-200 bg-blue-50 hover:bg-blue-100 text-blue-600 hover:text-blue-700 dark:border-blue-800 dark:bg-blue-950/50 dark:hover:bg-blue-950 dark:text-blue-400 transition-all duration-200 flex items-center gap-2"
          >
            <ArrowLeft size={14} />
            <span className="text-xs font-medium">Back</span>
          </Button>
        )}

        {/* Toggle Dev Panel Button - Enhanced styling */}
        {currentView === "chat" && (
          <Button
            variant="outline"
            size="sm"
            onClick={onToggleDevPanel}
            title={isDevPanelOpen ? "Hide Dev Panel" : "Show Dev Panel"}
            className={cn(
              "h-8 w-8 p-0 rounded-lg transition-all duration-200",
              isDevPanelOpen
                ? "border-purple-200 bg-purple-50 hover:bg-purple-100 text-purple-600 hover:text-purple-700 dark:border-purple-800 dark:bg-purple-950/50 dark:hover:bg-purple-950 dark:text-purple-400"
                : "border-muted bg-muted/50 hover:bg-muted text-muted-foreground hover:text-foreground"
            )}
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
