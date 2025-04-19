import React from "react";
import { BotMessageSquare, Server, Bug } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ServerStatus } from "@/types";

type HeaderProps = {
  serverStatus: ServerStatus;
  showLogs: boolean;
  setShowLogs: (show: boolean) => void;
};

const Header: React.FC<HeaderProps> = ({ serverStatus, showLogs, setShowLogs }) => {
  return (
    <header className="flex justify-between items-center mb-4 flex-shrink-0 border-b pb-2">
      <h1 className="text-xl font-semibold flex items-center gap-2">
        <BotMessageSquare size={24} /> DotDot AI Assistant
      </h1>
      <div className="flex items-center gap-3">
        {/* Status Indicator */}
        <div className="flex items-center gap-1 text-sm">
          <Server
            size={16}
            className={cn(
              serverStatus === "connected"
                ? "text-green-500"
                : serverStatus === "error"
                ? "text-red-500"
                : "text-yellow-500 animate-pulse"
            )}
          />
          {serverStatus === "connected"
            ? "Connected"
            : serverStatus === "error"
            ? "Connection Error"
            : "Connecting..."}
        </div>
        <Button
          variant="outline"
          size="icon"
          onClick={() => setShowLogs(!showLogs)}
          title={showLogs ? "Hide Logs" : "Show Logs"}
        >
          <Bug size={18} />
        </Button>
      </div>
    </header>
  );
};

export default Header;