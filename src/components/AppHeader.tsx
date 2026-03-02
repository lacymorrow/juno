import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { isDevelopment } from "@/lib";
import {
  ArrowLeft,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  SquarePen,
} from "lucide-react";

// Type for view state
export type AppView = "chat" | "devtools" | "permissions";

interface AppHeaderProps {
  serverStatus: "connected" | "connecting" | "error";
  isProcessing: boolean;
  currentView: AppView;
  isDevPanelOpen: boolean;
  onViewChange: (view: AppView) => void;
  onToggleDevPanel: () => void;
  onNewChat?: () => void;
}

export function AppHeader({
  serverStatus,
  isProcessing,
  currentView,
  isDevPanelOpen,
  onViewChange,
  onToggleDevPanel,
  onNewChat,
}: AppHeaderProps) {
  const [isDevMode, setIsDevMode] = useState(false);

  useEffect(() => {
    isDevelopment().then(setIsDevMode).catch(() => setIsDevMode(false));
  }, []);

  const handleOpenSettings = () => {
    invoke("open_settings_window").catch((err) =>
      console.error("Failed to open settings:", err),
    );
  };

  return (
    <header className="flex items-center justify-between py-1 px-3 border-b border-border/50 min-h-[36px]">
      <div className="flex items-center gap-2 flex-shrink-0">
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <span className="text-sm font-medium tracking-tight text-foreground/80">
            Juno
          </span>
          <Tooltip>
            <TooltipTrigger asChild>
              <div
                className={cn(
                  "w-1.5 h-1.5 rounded-full cursor-default",
                  serverStatus === "connected"
                    ? "bg-green-500"
                    : serverStatus === "error"
                    ? "bg-red-500"
                    : "bg-yellow-500"
                )}
              />
            </TooltipTrigger>
            <TooltipContent side="bottom" sideOffset={4}>
              {serverStatus === "connected"
                ? "Backend connected"
                : serverStatus === "error"
                ? "Backend connection error — check logs"
                : "Connecting to backend..."}
            </TooltipContent>
          </Tooltip>
        </div>
      </div>

      <div className="flex items-center gap-1 flex-shrink-0">
        {/* New Chat Button - only show in chat view */}
        {currentView === "chat" && onNewChat && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onNewChat}
            title="New Chat"
            className="h-7 w-7 p-0"
            disabled={isProcessing}
          >
            <SquarePen size={14} />
          </Button>
        )}
        {/* Back Button - show for devtools, permissions views */}
        {(currentView === "devtools" || currentView === "permissions") && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onViewChange("chat")}
            title="Back to Chat"
            className="h-7 w-7 p-0"
          >
            <ArrowLeft size={14} />
          </Button>
        )}
        {/* Settings Button */}
        {currentView === "chat" && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleOpenSettings}
            title="Settings"
            className="h-7 w-7 p-0"
          >
            <Settings size={14} />
          </Button>
        )}
        {/* Toggle Dev Panel Button - dev mode only */}
        {isDevMode && currentView === "chat" && (
          <Button
            variant="ghost"
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
