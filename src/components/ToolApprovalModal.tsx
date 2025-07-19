import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { safeUnlisten } from "@/lib/tauri-event-utils";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  Clock,
  Shield,
} from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";

interface ToolApprovalRequest {
  tool_name: string;
  tool_id: string;
  tool_input: any;
  description: string;
  timestamp: number;
}

export default function ToolApprovalModal() {
  const [isOpen, setIsOpen] = useState(false);
  const [currentRequest, setCurrentRequest] =
    useState<ToolApprovalRequest | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);

  useEffect(() => {
    // Listen for tool approval requests
    const unlistenPromise = listen<ToolApprovalRequest>(
      "tool-approval-request",
      (event) => {
        console.log("Tool approval request received:", event.payload);
        setCurrentRequest(event.payload);
        setIsOpen(true);
      }
    );

    return () => {
      unlistenPromise
        .then((unlisten) => safeUnlisten(unlisten))
        .catch((error) => {
          console.debug("Tool approval listener cleanup error (safe to ignore):", error);
        });
    };
  }, []);

  const handleApprove = async () => {
    if (!currentRequest) return;

    setIsProcessing(true);
    try {
      const success = await invoke<boolean>("approve_tool_execution", {
        toolId: currentRequest.tool_id,
      });

      if (success) {
        toast.success(`Tool "${currentRequest.tool_name}" approved`);
        setIsOpen(false);
        setCurrentRequest(null);
      } else {
        toast.error("Failed to approve tool execution");
      }
    } catch (error) {
      console.error("Error approving tool:", error);
      toast.error("Failed to approve tool execution");
    } finally {
      setIsProcessing(false);
    }
  };

  const handleDeny = async () => {
    if (!currentRequest) return;

    setIsProcessing(true);
    try {
      const success = await invoke<boolean>("deny_tool_execution", {
        toolId: currentRequest.tool_id,
      });

      if (success) {
        toast.success(`Tool "${currentRequest.tool_name}" denied`);
        setIsOpen(false);
        setCurrentRequest(null);
      } else {
        toast.error("Failed to deny tool execution");
      }
    } catch (error) {
      console.error("Error denying tool:", error);
      toast.error("Failed to deny tool execution");
    } finally {
      setIsProcessing(false);
    }
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  const getToolInputPreview = (input: any) => {
    if (!input) return "No parameters";

    try {
      const str = JSON.stringify(input, null, 2);
      // Truncate if too long
      if (str.length > 200) {
        return str.substring(0, 200) + "...";
      }
      return str;
    } catch {
      return String(input);
    }
  };

  if (!currentRequest) return null;

  return (
    <Dialog open={isOpen} onOpenChange={() => {}}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="h-5 w-5 text-amber-600" />
            Tool Approval Required
          </DialogTitle>
          <DialogDescription>
            The AI agent wants to execute a tool and requires your approval.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Tool Information */}
          <Card>
            <CardContent className="pt-4">
              <div className="space-y-3">
                <div>
                  <div className="flex items-center gap-2 mb-1">
                    <span className="font-medium">Tool:</span>
                    <Badge variant="outline">{currentRequest.tool_name}</Badge>
                  </div>
                  <p className="text-sm text-gray-600">
                    {currentRequest.description}
                  </p>
                </div>

                <div>
                  <div className="flex items-center gap-2 mb-1">
                    <Clock className="h-4 w-4 text-gray-500" />
                    <span className="text-sm text-gray-500">
                      Requested at {formatTimestamp(currentRequest.timestamp)}
                    </span>
                  </div>
                </div>

                {/* Tool Parameters */}
                <div>
                  <span className="font-medium text-sm">Parameters:</span>
                  <div className="mt-1 p-2 bg-gray-50 rounded text-xs font-mono">
                    <pre className="whitespace-pre-wrap">
                      {getToolInputPreview(currentRequest.tool_input)}
                    </pre>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Warning */}
          <div className="flex items-start gap-2 p-3 bg-amber-50 border border-amber-200 rounded-lg">
            <AlertTriangle className="h-4 w-4 text-amber-600 mt-0.5" />
            <div className="text-sm text-amber-800">
              <div className="font-medium">Review Carefully</div>
              <div className="mt-1">
                Make sure you understand what this tool will do before
                approving. Some tools may make changes to your system or files.
              </div>
            </div>
          </div>
        </div>

        <DialogFooter className="gap-2">
          <Button
            variant="outline"
            onClick={handleDeny}
            disabled={isProcessing}
            className="flex items-center gap-2"
          >
            <XCircle className="h-4 w-4" />
            Deny
          </Button>
          <Button
            onClick={handleApprove}
            disabled={isProcessing}
            className="flex items-center gap-2"
          >
            <CheckCircle className="h-4 w-4" />
            {isProcessing ? "Processing..." : "Approve"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
