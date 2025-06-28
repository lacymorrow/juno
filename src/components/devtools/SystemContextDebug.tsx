import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import { invokeCommand } from "@/lib/utils";
import type { LoadingStates } from "@/types/devtools";
import { Download, Eye, FileText } from "lucide-react";
import React, { useState } from "react";
import { toast } from "sonner";

interface SystemContextDebugProps {
  loadingStates: LoadingStates;
  setLoadingStates: React.Dispatch<React.SetStateAction<LoadingStates>>;
}

const SystemContextDebug: React.FC<SystemContextDebugProps> = ({
  loadingStates,
  setLoadingStates,
}) => {
  const [systemContext, setSystemContext] = useState<string>("");
  const [showContext, setShowContext] = useState<boolean>(false);

  const handleGetSystemContext = async () => {
    setLoadingStates((prev) => ({ ...prev, testSystemContext: true }));

    try {
      const context = await invokeCommand<string>("test_system_context", {});
      setSystemContext(context);
      setShowContext(true);
      toast.success("System context gathered successfully");
    } catch (error) {
      console.error("Failed to get system context:", error);
      toast.error(`Failed to get system context: ${error}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, testSystemContext: false }));
    }
  };

  const handleDownloadContext = () => {
    if (!systemContext) {
      toast.error("No system context to download. Please fetch it first.");
      return;
    }

    try {
      const blob = new Blob([systemContext], { type: "text/plain" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `juno-system-context-${new Date()
        .toISOString()
        .replace(/[:.]/g, "-")}.txt`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
      toast.success("System context downloaded successfully");
    } catch (error) {
      console.error("Failed to download system context:", error);
      toast.error("Failed to download system context");
    }
  };

  const toggleContextView = () => {
    setShowContext(!showContext);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center space-x-2">
        <FileText className="h-4 w-4" />
        <Button
          onClick={handleGetSystemContext}
          disabled={loadingStates.testSystemContext}
          className="flex items-center space-x-2"
        >
          <Eye className="h-4 w-4" />
          <span>
            {loadingStates.testSystemContext
              ? "Getting Context..."
              : "Get System Context"}
          </span>
        </Button>

        {systemContext && (
          <>
            <Button
              onClick={toggleContextView}
              variant="outline"
              className="flex items-center space-x-2"
            >
              <Eye className="h-4 w-4" />
              <span>{showContext ? "Hide Context" : "View Context"}</span>
            </Button>

            <Button
              onClick={handleDownloadContext}
              variant="outline"
              className="flex items-center space-x-2"
            >
              <Download className="h-4 w-4" />
              <span>Download</span>
            </Button>
          </>
        )}
      </div>

      {showContext && systemContext && (
        <div className="space-y-2">
          <div className="text-sm text-muted-foreground">
            System Context ({systemContext.length.toLocaleString()} characters):
          </div>
          <ScrollArea className="h-96 w-full rounded-md border">
            <Textarea
              value={systemContext}
              readOnly
              className="min-h-[360px] resize-none border-0 font-mono text-xs"
              placeholder="System context will appear here..."
            />
          </ScrollArea>
        </div>
      )}

      {!systemContext && (
        <div className="text-sm text-muted-foreground">
          Click "Get System Context" to view the current system context that the
          AI agent receives. This includes information about focused windows,
          running applications, clipboard content, system performance, and
          voice/audio state.
        </div>
      )}
    </div>
  );
};

export default SystemContextDebug;
