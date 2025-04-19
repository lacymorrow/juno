import React from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { LogEntry } from "@/types";

type LogsPanelProps = {
  logs: LogEntry[];
  logsEndRef: React.RefObject<HTMLDivElement>;
  getLogColorClass: (level: string) => string;
  formatTimestamp: (timestamp: number) => string;
};

const LogsPanel: React.FC<LogsPanelProps> = ({
  logs,
  logsEndRef,
  getLogColorClass,
  formatTimestamp,
}) => {
  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0">
        <CardTitle className="text-lg">Logs</CardTitle>
      </CardHeader>
      <CardContent className="flex-grow overflow-hidden p-0">
        <ScrollArea className="h-full p-3">
          {logs.map((log, index) => (
            <div
              key={index}
              className={cn(
                "text-xs mb-1 font-mono whitespace-pre-wrap",
                getLogColorClass(log.level)
              )}
            >
              <span className="text-muted-foreground mr-1">
                [{formatTimestamp(log.timestamp)}]
              </span>
              <span className="font-semibold mr-1">
                [{log.level.toUpperCase()}]
              </span>
              {log.message}
            </div>
          ))}
          <div ref={logsEndRef} />
        </ScrollArea>
      </CardContent>
    </Card>
  );
};

export default LogsPanel;