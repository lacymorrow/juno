import {
  MixedContentRenderer,
} from "@/components/ui/mixed-content-renderer";
import {
  Message,
  MessageContent,
  MessageActions,
  MessageAction,
  MessageResponse,
  MessageToolbar,
} from "@/components/ai-elements/message";
import {
  Reasoning,
  ReasoningTrigger,
  ReasoningContent,
} from "@/components/ai-elements/reasoning";
import {
  Tool,
  ToolHeader,
  ToolContent,
  ToolInput,
  ToolOutput,
} from "@/components/ai-elements/tool";
import { Shimmer } from "@/components/ai-elements/shimmer";
import { Button } from "@/components/ui/button";
import {
  Code,
  Copy,
  FileText,
  Volume2,
  ChevronDown,
  ChevronRight,
  CheckCircle,
  XCircle,
  WifiOff,
} from "lucide-react";
import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { UI } from "@/lib/constants.generated";

export type { ChatMessage } from "@/types/chat";
import type { ChatMessage } from "@/types/chat";

interface ChatMessageProps {
  msg: ChatMessage;
  index: number;
  copyingMessageId: string | null;
  savingMessageId: string | null;
  onCopyResponse: (content: string, index: number) => void;
  onSaveResponse: (
    content: string,
    format: "html" | "markdown",
    index: number
  ) => void;
  onApprovalUpdate?: (toolId: string, state: "approved" | "denied") => void;
}

// Compact accordion for TTS spoken content — closed by default, supplementary info
function TTSContentDisplay({
  ttsMetadata,
}: {
  ttsMetadata: ChatMessage["tts_metadata"];
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  if (!ttsMetadata?.has_spoken_content || !ttsMetadata.tts_parts.length) {
    return null;
  }

  // Combine all parts into one spoken text for display
  const spokenText = ttsMetadata.total_spoken_text?.trim()
    || ttsMetadata.tts_parts.map((p) => p.trim()).join(" ");

  return (
    <div className="mt-1">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground/70 hover:text-muted-foreground transition-colors"
      >
        {isExpanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <Volume2 className="h-3 w-3" />
        <span>Spoken aloud</span>
      </button>

      {isExpanded && (
        <div className="mt-1 pl-5 text-xs text-muted-foreground/80 italic leading-relaxed">
          {spokenText}
        </div>
      )}
    </div>
  );
}

// Status badge for empty assistant messages (e.g., TTS-only or agent state changes)
function AgentStatusBadge({ agentState }: { agentState?: string }) {
  if (agentState === UI.AGENT_STATUS_FINISHED || agentState === UI.AGENT_STATUS_SUCCESS) {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <CheckCircle className="h-3 w-3 text-green-600" />
        <span>Complete</span>
      </span>
    );
  }
  if (agentState === UI.AGENT_STATUS_FAILED || agentState === UI.AGENT_STATUS_ERROR) {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <XCircle className="h-3 w-3 text-red-500" />
        <span>Failed</span>
      </span>
    );
  }
  if (agentState === UI.AGENT_STATUS_CANCELLED) {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <XCircle className="h-3 w-3 text-yellow-500" />
        <span>Cancelled</span>
      </span>
    );
  }
  if (agentState === UI.AGENT_STATUS_OFFLINE) {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <WifiOff className="h-3 w-3" />
        <span>Offline</span>
      </span>
    );
  }
  // Default: generic "done" for unknown or missing states
  return (
    <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
      <CheckCircle className="h-3 w-3 text-muted-foreground/50" />
      <span>Done</span>
    </span>
  );
}

export function ChatMessageComponent({
  msg,
  index,
  copyingMessageId,
  savingMessageId,
  onCopyResponse,
  onSaveResponse,
  onApprovalUpdate,
}: ChatMessageProps) {
  // Inline tool approval handlers
  const handleApprove = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>("approve_tool_execution", { toolId });
      if (success) {
        toast.success("Tool approved");
        onApprovalUpdate?.(toolId, "approved");
      } else {
        toast.error("Failed to approve tool execution");
      }
    } catch (error) {
      console.error("Error approving tool:", error);
      toast.error("Failed to approve tool execution");
    }
  }, [onApprovalUpdate]);

  const handleDeny = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>("deny_tool_execution", { toolId });
      if (success) {
        toast.success("Tool denied");
        onApprovalUpdate?.(toolId, "denied");
      } else {
        toast.error("Failed to deny tool execution");
      }
    } catch (error) {
      console.error("Error denying tool:", error);
      toast.error("Failed to deny tool execution");
    }
  }, [onApprovalUpdate]);

  // Thinking messages — Reasoning component with auto-open/close and duration tracking
  if (msg.role === "thinking") {
    return (
      <div className="flex justify-start w-full">
        <Reasoning isStreaming={msg.isStreaming}>
          <ReasoningTrigger />
          <ReasoningContent>{msg.content}</ReasoningContent>
        </Reasoning>
      </div>
    );
  }

  // Tool call requests — with inline approval if pending
  if (msg.role === "tool_call_request") {
    const toolState = msg.approval_state === "pending"
      ? "approval-requested" as const
      : msg.approval_state === "denied"
        ? "output-denied" as const
        : "input-available" as const;

    return (
      <div className="flex justify-start w-full">
        <Tool>
          <ToolHeader
            type="dynamic-tool"
            state={toolState}
            toolName={msg.tool_name || "unknown"}
            title={msg.tool_name}
          />
          <ToolContent>
            {msg.tool_args && <ToolInput input={msg.tool_args} />}
            {msg.approval_state === "pending" && msg.tool_id && (
              <div className="flex items-center gap-2 pt-2 border-t">
                <Button
                  size="sm"
                  onClick={() => handleApprove(msg.tool_id!)}
                >
                  <CheckCircle className="mr-1.5 h-4 w-4" />
                  Approve
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => handleDeny(msg.tool_id!)}
                >
                  <XCircle className="mr-1.5 h-4 w-4" />
                  Deny
                </Button>
              </div>
            )}
          </ToolContent>
        </Tool>
      </div>
    );
  }

  // Tool call results
  if (msg.role === "tool_call_result") {
    const resultState = (msg.success ?? true)
      ? "output-available" as const
      : "output-error" as const;

    return (
      <div className="flex justify-start w-full">
        <Tool>
          <ToolHeader
            type="dynamic-tool"
            state={resultState}
            toolName={msg.tool_name || "unknown"}
            title={msg.tool_name}
          />
          <ToolContent>
            <ToolOutput
              output={msg.tool_output}
              errorText={(msg.success ?? true) ? undefined : msg.content}
            />
            {msg.screenshot_base64 && (
              <div className="mt-2">
                <img
                  src={`data:image/png;base64,${msg.screenshot_base64}`}
                  alt="Tool screenshot"
                  className="max-h-[300px] rounded border border-border"
                />
              </div>
            )}
          </ToolContent>
        </Tool>
      </div>
    );
  }

  const from = msg.role === "user" ? "user" : "assistant";

  return (
    <Message key={`msg-${index}-${msg.timestamp || Date.now()}`} from={from}>
      <MessageContent>
        {/* Main content rendering */}
        {msg.role === "assistant" &&
        (!msg.content || msg.content.trim() === "") ? (
          <AgentStatusBadge agentState={msg.agent_state} />
        ) : msg.role === "assistant" && msg.content && msg.isJsx ? (
          <MixedContentRenderer content={msg.content} isStreaming={msg.isStreaming} />
        ) : msg.role === "assistant" && msg.content ? (
          <MessageResponse>{msg.content}</MessageResponse>
        ) : msg.role === "user" ? (
          <span>{msg.content}</span>
        ) : (
          msg.content
        )}

        {/* TTS spoken content — compact accordion, closed by default */}
        {msg.role === "assistant" && !msg.isStreaming && (
          <TTSContentDisplay ttsMetadata={msg.tts_metadata} />
        )}

        {msg.screenshot_base64 && (
          <div className="mt-2 border-t pt-2">
            <div className="text-xs text-muted-foreground mb-1">
              {msg.role === "system"
                ? "Screenshot captured by AI:"
                : "Screenshot:"}
            </div>
            <div className="relative">
              <img
                src={`data:image/png;base64,${msg.screenshot_base64}`}
                alt="Screenshot"
                className="rounded w-full object-contain max-h-[300px] border border-border shadow-sm"
              />
            </div>
          </div>
        )}

        {msg.isStreaming && (
          <Shimmer as="span" duration={1.5}>...</Shimmer>
        )}
      </MessageContent>

      {/* Action toolbar for assistant messages */}
      {msg.role === "assistant" &&
        msg.content &&
        msg.content.trim() !== "" &&
        !msg.isStreaming && (
          <MessageToolbar className="opacity-0 group-hover:opacity-100 transition-opacity duration-200">
            <MessageActions>
              <MessageAction
                tooltip="Copy response"
                onClick={() => onCopyResponse(msg.content, index)}
                disabled={copyingMessageId === `copy-${index}`}
                className="h-7 w-7"
              >
                {copyingMessageId === `copy-${index}` ? (
                  <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                ) : (
                  <Copy size={14} />
                )}
              </MessageAction>
              <MessageAction
                tooltip="Save as HTML"
                onClick={() => onSaveResponse(msg.content, "html", index)}
                disabled={savingMessageId === `save-html-${index}`}
                className="h-7 w-7"
              >
                {savingMessageId === `save-html-${index}` ? (
                  <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                ) : (
                  <Code size={14} />
                )}
              </MessageAction>
              <MessageAction
                tooltip="Save as Markdown"
                onClick={() => onSaveResponse(msg.content, "markdown", index)}
                disabled={savingMessageId === `save-markdown-${index}`}
                className="h-7 w-7"
              >
                {savingMessageId === `save-markdown-${index}` ? (
                  <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                ) : (
                  <FileText size={14} />
                )}
              </MessageAction>
            </MessageActions>
          </MessageToolbar>
        )}
    </Message>
  );
}
