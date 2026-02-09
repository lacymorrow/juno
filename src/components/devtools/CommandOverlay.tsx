import { useState } from "react";
import { useEventListener } from "@/hooks/useEventListener";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CheckCircle, XCircle, Loader } from "lucide-react";
import { EVENTS } from "@/lib/constants.generated";

type CommandStatus = "executing" | "completed" | "failed";

interface Command {
  command: string;
  status: CommandStatus;
}

export const CommandOverlay = () => {
  const [commands, setCommands] = useState<Record<string, Command>>({});

  useEventListener<{ id: string; command: string }>(
    EVENTS.TOOLS_COMMAND_EXECUTION_START,
    (payload) => {
      const { id, command } = payload;
      setCommands((prevCommands) => ({
        ...prevCommands,
        [id]: { command, status: "executing" },
      }));
    }
  );

  useEventListener<{ id: string; success: boolean }>(
    EVENTS.TOOLS_COMMAND_EXECUTION_END,
    (payload) => {
      const { id, success } = payload;
      const newStatus = success ? "completed" : "failed";

      setCommands((prevCommands) => {
        const updatedCommands = { ...prevCommands };
        const existingCommand = updatedCommands[id];
        if (existingCommand) {
          updatedCommands[id] = { ...existingCommand, status: newStatus };
          if (newStatus === "completed" || newStatus === "failed") {
            setTimeout(() => {
              setCommands((prev) => {
                const newCommands = { ...prev };
                delete newCommands[id];
                return newCommands;
              });
            }, 3000);
          }
        }
        return updatedCommands;
      });
    }
  );

  const getStatusIcon = (status: CommandStatus) => {
    switch (status) {
      case "executing":
        return <Loader className="animate-spin h-5 w-5 text-blue-500" />;
      case "completed":
        return <CheckCircle className="h-5 w-5 text-green-500" />;
      case "failed":
        return <XCircle className="h-5 w-5 text-red-500" />;
    }
  };

  if (Object.keys(commands).length === 0) {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 z-50">
      <Card className="w-80 bg-background/80 backdrop-blur-sm">
        <CardHeader>
          <CardTitle className="text-sm font-medium">Command Status</CardTitle>
        </CardHeader>
        <CardContent>
          <ul className="space-y-2">
            {Object.entries(commands).map(([id, { command, status }]) => (
              <li
                key={id}
                className="flex items-center justify-between text-xs"
              >
                <span className="truncate">{command}</span>
                {getStatusIcon(status)}
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>
    </div>
  );
};
