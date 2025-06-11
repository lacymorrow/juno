import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

type CommandStatus = "executing" | "completed" | "failed";

type CommandInfo = {
  command: string;
  status: CommandStatus;
  id: number;
  timestamp: number;
  duration?: number;
  error?: string;
};

const CommandOverlay = () => {
  const [commands, setCommands] = useState<CommandInfo[]>([]);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem("juno-show-command-overlay") === "true"
  );

  // Check localStorage periodically for setting changes
  useEffect(() => {
    const checkSettings = () => {
      const enabled =
        localStorage.getItem("juno-show-command-overlay") === "true";
      setIsEnabled(enabled);
    };

    // Check immediately and then every second
    checkSettings();
    const interval = setInterval(checkSettings, 1000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    // Only listen for events if overlay is enabled
    if (!isEnabled) return;

    // Listen for command execution start events
    const unlistenStart = listen<{ command: string; id: number }>(
      "command-execution-start",
      (event) => {
        const { command, id } = event.payload;
        const newCommand: CommandInfo = {
          command,
          status: "executing",
          id,
          timestamp: Date.now(),
        };

        setCommands((prevCommands) => [...prevCommands, newCommand]);
      }
    );

    // Listen for command execution end events
    const unlistenEnd = listen<{
      id: number;
      success: boolean;
      duration?: number;
      error?: string;
    }>("command-execution-end", (event) => {
      const { id, success, duration, error } = event.payload;

      setCommands((prevCommands) =>
        prevCommands.map((cmd) =>
          cmd.id === id
            ? {
                ...cmd,
                status: success ? "completed" : "failed",
                duration,
                error,
              }
            : cmd
        )
      );
    });

    return () => {
      // Cleanup listeners when component unmounts or is disabled
      Promise.all([unlistenStart, unlistenEnd]).then(
        ([unlistenStartFn, unlistenEndFn]) => {
          unlistenStartFn();
          unlistenEndFn();
        }
      );
    };
  }, [isEnabled]);

  // Clean up old commands (remove completed/failed commands after delay)
  useEffect(() => {
    if (commands.length === 0 || !isEnabled) return;

    const cleanupTimeout = setTimeout(() => {
      const now = Date.now();
      setCommands((prevCommands) =>
        prevCommands.filter((cmd) => {
          // Keep executing commands
          if (cmd.status === "executing") return true;
          // Remove completed/failed commands older than 3 seconds
          return now - cmd.timestamp < 3000;
        })
      );
    }, 100);

    return () => clearTimeout(cleanupTimeout);
  }, [commands, isEnabled]);

  // Don't render anything if disabled
  if (!isEnabled) {
    return null;
  }

  // Get status icon and color
  const getStatusDisplay = (status: CommandStatus) => {
    switch (status) {
      case "executing":
        return {
          icon: "⏳",
          color: "#3b82f6",
          bgColor: "rgba(59, 130, 246, 0.1)",
        };
      case "completed":
        return {
          icon: "✅",
          color: "#10b981",
          bgColor: "rgba(16, 185, 129, 0.1)",
        };
      case "failed":
        return {
          icon: "❌",
          color: "#ef4444",
          bgColor: "rgba(239, 68, 68, 0.1)",
        };
    }
  };

  // Format command name for display
  const formatCommandName = (command: string) => {
    // Remove common prefixes and make more readable
    return command
      .replace(/^(dev_|qa_|test_)/, "")
      .replace(/_/g, " ")
      .replace(/\b\w/g, (l) => l.toUpperCase());
  };

  return (
    <div
      className="command-overlay"
      style={{
        position: "fixed",
        top: "20px",
        left: "20px",
        pointerEvents: "none", // Allow clicks to pass through
        zIndex: 999997, // High z-index but below key press overlay
        display: "flex",
        flexDirection: "column",
        gap: "6px",
        maxWidth: "280px",
      }}
    >
      {commands.map((command, index) => {
        const { icon, color } = getStatusDisplay(command.status);
        return (
          <div
            key={command.id}
            className="command-indicator"
            style={{
              backgroundColor: "rgba(0, 0, 0, 0.85)",
              color: "white",
              padding: "8px 12px",
              borderRadius: "8px",
              fontSize: "13px",
              fontFamily: "ui-sans-serif, system-ui, sans-serif",
              border: `1px solid ${color}`,
              animation:
                command.status === "executing"
                  ? "command-pulse 2s infinite ease-in-out"
                  : `command-fade-out 3s ease-out forwards`,
              animationDelay: `${index * 100}ms`,
              backdropFilter: "blur(6px)",
              boxShadow: `0 2px 8px ${color}20`,
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <span style={{ fontSize: "16px" }}>{icon}</span>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontWeight: "500",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {formatCommandName(command.command)}
                </div>
                {command.duration && (
                  <div
                    style={{
                      fontSize: "11px",
                      opacity: 0.7,
                      marginTop: "2px",
                    }}
                  >
                    {command.duration}ms
                  </div>
                )}
                {command.error && (
                  <div
                    style={{
                      fontSize: "11px",
                      color: "#fca5a5",
                      marginTop: "2px",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {command.error}
                  </div>
                )}
              </div>
            </div>
          </div>
        );
      })}
      <style>{`
        @keyframes command-pulse {
          0% {
            opacity: 1;
            transform: scale(1);
          }
          50% {
            opacity: 0.8;
            transform: scale(1.02);
          }
          100% {
            opacity: 1;
            transform: scale(1);
          }
        }

        @keyframes command-fade-out {
          0% {
            opacity: 1;
            transform: translateX(0);
          }
          70% {
            opacity: 1;
            transform: translateX(0);
          }
          100% {
            opacity: 0;
            transform: translateX(-20px);
          }
        }
      `}</style>
    </div>
  );
};

export default CommandOverlay;
