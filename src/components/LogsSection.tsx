import React from "react";
import { Bug } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { LogEntry } from "@/types";

type LogsSectionProps = {
  logs: LogEntry[];
  showLogs: boolean;
  setShowLogs: (show: boolean) => void;
  logsEndRef: React.RefObject<HTMLDivElement>;
  getLogColorClass: (level: string) => string;
  formatTimestamp: (timestamp: number) => string;
};

const LogsSection: React.FC<LogsSectionProps> = ({
  logs,
  showLogs,
  setShowLogs,
  logsEndRef,
  getLogColorClass,
  formatTimestamp,
}) => {
  return (
    <div className="mt-auto p-4 border-t">
      <div className="flex justify-between items-center mb-2">
        <h2 className="text-lg font-semibold">Logs</h2>
        <Button
          variant="outline"
          size="icon"
          onClick={() => setShowLogs(!showLogs)}
        >
          <Bug className="h-4 w-4" />
        </Button>
      </div>
      {showLogs && (
        <ScrollArea className="h-40 w-full rounded-md border p-2 bg-gray-50 dark:bg-gray-900">
          {logs.map((log, index) => (
            <div
              key={index}
              className={cn(
                "text-xs mb-1 flex items-start",
                getLogColorClass(log.level)
              )}
            >
              <span className="font-mono mr-2 flex-shrink-0">
                [{formatTimestamp(log.timestamp)}]
              </span>
              <span className="font-mono mr-1 flex-shrink-0">
                [{log.level.toUpperCase()}]
              </span>
              <span className="break-words">{log.message}</span>
            </div>
          ))}
          <div ref={logsEndRef} /> {/* Scroll anchor */}
        </ScrollArea>
      )}
    </div>
  );
};

export default LogsSection;