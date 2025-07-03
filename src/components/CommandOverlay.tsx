import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { EVENTS } from "@/lib/constants.generated";

interface CommandInfo {
  id: string;
  command: string;
  timestamp: number;
  status: "executing" | "completed" | "failed";
  duration?: number;
  error?: string;
}

interface CommandStartPayload {
  id: string;
  command: string;
}

interface CommandEndPayload {
  id: string;
  success: boolean;
  duration?: number;
  error?: string;
}

export default function CommandOverlay() {
  const [commands, setCommands] = useState<CommandInfo[]>([]);
  const [isVisible, setIsVisible] = useState(false);
  const [showCommandOverlay, setShowCommandOverlay] = useState(true);

  // Load settings on mount
  useEffect(() => {
    const loadSettings = async () => {
      try {
        // For now, using a simple approach - could be extended to use centralized settings
        const enabled = await invoke<boolean>(
          "get_command_overlay_enabled"
        ).catch(() => true);
        setShowCommandOverlay(enabled);
      } catch (error) {
        console.warn(
          "Failed to load command overlay settings, using default (enabled):",
          error
        );
        setShowCommandOverlay(true);
      }
    };

    loadSettings();
  }, []);

  // Listen for command execution events
  useEffect(() => {
    if (!showCommandOverlay) return;

    let unlistenStart: (() => void) | null = null;
    let unlistenEnd: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for command execution start
      unlistenStart = await listen<CommandStartPayload>(
        EVENTS.TOOLS_COMMAND_EXECUTION_START,
        (event) => {
          const { id, command } = event.payload;
          const newCommand: CommandInfo = {
            id,
            command,
            timestamp: Date.now(),
            status: "executing",
          };

          setCommands((prev) => {
            // Remove any existing command with same ID and add new one
            const filtered = prev.filter((cmd) => cmd.id !== id);
            return [...filtered, newCommand];
          });

          setIsVisible(true);
        }
      );

      // Listen for command execution end
      unlistenEnd = await listen<CommandEndPayload>(
        EVENTS.TOOLS_COMMAND_EXECUTION_END,
        (event) => {
          const { id, success, duration, error } = event.payload;

          setCommands((prev) =>
            prev.map((cmd) =>
              cmd.id === id
                ? {
                    ...cmd,
                    status: success ? "completed" : "failed",
                    duration,
                    error: error || undefined,
                  }
                : cmd
            )
          );

          // Auto-hide after 5 seconds for completed/failed commands
          setTimeout(() => {
            setCommands((prev) => {
              const filtered = prev.filter((cmd) => cmd.id !== id);
              if (filtered.length === 0) {
                setIsVisible(false);
              }
              return filtered;
            });
          }, 5000);
        }
      );
    };

    setupListeners();

    return () => {
      if (unlistenStart) unlistenStart();
      if (unlistenEnd) unlistenEnd();
    };
  }, [showCommandOverlay]);

  const getStatusDisplay = (status: string) => {
    switch (status) {
      case "executing":
        return {
          icon: "⏳",
          color: "text-blue-600",
          bgColor: "bg-blue-50 border-blue-200",
        };
      case "completed":
        return {
          icon: "✅",
          color: "text-green-600",
          bgColor: "bg-green-50 border-green-200",
        };
      case "failed":
        return {
          icon: "❌",
          color: "text-red-600",
          bgColor: "bg-red-50 border-red-200",
        };
      default:
        return {
          icon: "⏳",
          color: "text-gray-600",
          bgColor: "bg-gray-50 border-gray-200",
        };
    }
  };

  // Don't render if disabled or no commands
  if (!showCommandOverlay || !isVisible || commands.length === 0) {
    return null;
  }

  return (
    <div className="fixed top-4 right-4 z-50 space-y-2">
      {commands.slice(-5).map((command) => {
        const statusDisplay = getStatusDisplay(command.status);
        return (
          <div
            key={command.id}
            className={`
              p-3 rounded-lg border shadow-lg
              ${statusDisplay.bgColor} ${statusDisplay.color}
              animate-in slide-in-from-top-2 duration-200
              max-w-sm
            `}
          >
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">
                {statusDisplay.icon} {command.command}
              </span>
              {command.duration !== undefined && command.duration > 0 && (
                <span className="text-xs opacity-75">
                  ({command.duration}ms)
                </span>
              )}
            </div>
            {command.error && (
              <div className="text-xs text-red-600 mt-1 max-w-xs truncate">
                {command.error}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
