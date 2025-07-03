import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface CommandInfo {
  id: number;
  command: string;
  timestamp: number;
  status: "executing" | "completed" | "failed";
  duration?: number;
  error?: string;
}

interface CommandEventPayload {
  id?: number;
  command?: string;
  success?: boolean;
  error?: string;
  duration?: number;
}

export default function CommandOverlay() {
  const [commands, setCommands] = useState<CommandInfo[]>([]);
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    const unlisten = listen<CommandEventPayload>("command-event", (event) => {
      const { id, command, success, error, duration } = event.payload;
      const timestamp = Date.now();

      const newCommand: CommandInfo = {
        id: id || timestamp,
        command: command || "Unknown command",
        timestamp,
        status: success ? "completed" : "failed",
        duration: duration || 0,
        error: error || undefined,
      };

      setCommands((prev) => [...prev, newCommand]);
      setIsVisible(true);

      // Auto-hide after 5 seconds
      setTimeout(() => {
        setIsVisible(false);
      }, 5000);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const getStatusDisplay = (status: string) => {
    switch (status) {
      case "executing":
        return { icon: "⏳", color: "text-blue-600", bgColor: "bg-blue-50" };
      case "completed":
        return { icon: "✅", color: "text-green-600", bgColor: "bg-green-50" };
      case "failed":
        return { icon: "❌", color: "text-red-600", bgColor: "bg-red-50" };
      default:
        return { icon: "⏳", color: "text-gray-600", bgColor: "bg-gray-50" };
    }
  };

  if (!isVisible || commands.length === 0) {
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
              animate-in slide-in-from-top-2
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
              <div className="text-xs text-red-600 mt-1">{command.error}</div>
            )}
          </div>
        );
      })}
    </div>
  );
}
