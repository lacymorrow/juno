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
import {
  Confirmation,
  ConfirmationRequest,
  ConfirmationAccepted,
  ConfirmationRejected,
  ConfirmationActions,
  ConfirmationAction,
} from "@/components/ai-elements/confirmation";
import { Shimmer } from "@/components/ai-elements/shimmer";
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
  Square,
  Play,
  AlertTriangle,
  ShieldAlert,
  Clock,
  AppWindow,
} from "lucide-react";
import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { UI } from "@/lib/constants.generated";

export type { ChatMessage } from "@/types/chat";
import type { ChatMessage } from "@/types/chat";

const ACTION_VERB: Record<string, string> = {
  left_click: "Clicked",
  right_click: "Right-clicked",
  double_click: "Double-clicked",
  middle_click: "Middle-clicked",
};

function formatAxActionTitle(msg: ChatMessage): string | undefined {
  if (!msg.ax_action) return undefined;
  const verb = ACTION_VERB[msg.ax_action] ?? msg.ax_action;
  if (msg.ax_grounded && msg.ax_role) {
    const label = msg.ax_label ? ` '${msg.ax_label}'` : "";
    return `${verb} ${msg.ax_role}${label}`;
  }
  if (msg.ax_screen_coordinate) {
    const [x, y] = msg.ax_screen_coordinate;
    return `${verb} at (${Math.round(x)}, ${Math.round(y)})`;
  }
  return `${verb}`;
}

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
  onContinuationUpdate?: (requestId: string, state: "stopped" | "continued") => void;
}

// Compact accordion for TTS spoken content
// Expanded by default when TTS is the only response content, collapsed otherwise
function TTSContentDisplay({
  ttsMetadata,
  defaultExpanded = false,
}: {
  ttsMetadata: ChatMessage["tts_metadata"];
  defaultExpanded?: boolean;
}) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

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

function ContinuationActions({
  requestId,
  state,
  onUpdate,
}: {
  requestId: string;
  state: "pending" | "stopped" | "continued";
  onUpdate?: (requestId: string, state: "stopped" | "continued") => void;
}) {
  const handleStop = useCallback(async () => {
    try {
      await invoke("respond_to_agent_continuation", {
        requestId,
        approved: false,
      });
      onUpdate?.(requestId, "stopped");
    } catch (error) {
      console.error("Failed to stop agent:", error);
    }
  }, [requestId, onUpdate]);

  const handleContinue = useCallback(async () => {
    try {
      await invoke("respond_to_agent_continuation", {
        requestId,
        approved: true,
        additionalSteps: 20,
      });
      onUpdate?.(requestId, "continued");
    } catch (error) {
      console.error("Failed to continue agent:", error);
    }
  }, [requestId, onUpdate]);

  if (state === "stopped") {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <Square className="h-3 w-3 text-red-500" />
        <span>Stopped</span>
      </span>
    );
  }

  if (state === "continued") {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <Play className="h-3 w-3 text-green-600" />
        <span>Continued (+20 steps)</span>
      </span>
    );
  }

  return (
    <div className="flex items-center gap-2 mt-1.5">
      <button
        onClick={handleStop}
        className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors"
      >
        <Square className="h-3 w-3" />
        Stop
      </button>
      <button
        onClick={handleContinue}
        className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md bg-muted hover:bg-muted/80 text-muted-foreground transition-colors"
      >
        <Play className="h-3 w-3" />
        Continue (+20)
      </button>
    </div>
  );
}

type RiskLevel = "low" | "medium" | "high" | "critical";

const RISK_CONFIG: Record<
  RiskLevel,
  { label: string; className: string; Icon: React.ElementType }
> = {
  low: {
    label: "Low risk",
    className: "text-muted-foreground bg-muted/40",
    Icon: ShieldAlert,
  },
  medium: {
    label: "Medium risk",
    className: "text-yellow-700 dark:text-yellow-400 bg-yellow-50/60 dark:bg-yellow-950/30",
    Icon: AlertTriangle,
  },
  high: {
    label: "High risk",
    className: "text-orange-700 dark:text-orange-400 bg-orange-50/60 dark:bg-orange-950/30",
    Icon: AlertTriangle,
  },
  critical: {
    label: "Critical",
    className: "text-red-700 dark:text-red-400 bg-red-50/60 dark:bg-red-950/30",
    Icon: ShieldAlert,
  },
};

function RiskBadge({ level }: { level: RiskLevel }) {
  const { label, className, Icon } = RISK_CONFIG[level] ?? RISK_CONFIG.low;
  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium ${className}`}
    >
      <Icon className="size-3" />
      {label}
    </span>
  );
}

function ApprovalCountdown({
  timeoutSeconds,
  isPending,
}: {
  timeoutSeconds: number;
  isPending: boolean;
}) {
  const [timeLeft, setTimeLeft] = useState(timeoutSeconds);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!isPending) {
      if (intervalRef.current) clearInterval(intervalRef.current);
      return;
    }
    setTimeLeft(timeoutSeconds);
    intervalRef.current = setInterval(() => {
      setTimeLeft((t) => {
        if (t <= 1) {
          if (intervalRef.current) clearInterval(intervalRef.current);
          return 0;
        }
        return t - 1;
      });
    }, 1000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [isPending, timeoutSeconds]);

  if (!isPending) return null;

  const pct = (timeLeft / timeoutSeconds) * 100;
  const urgentColor =
    timeLeft <= 10
      ? "text-red-600 dark:text-red-400"
      : timeLeft <= 20
        ? "text-orange-600 dark:text-orange-400"
        : "text-muted-foreground";

  return (
    <div className="flex items-center gap-1.5 mt-1">
      <Clock className={`size-3 shrink-0 ${urgentColor}`} />
      <div className="flex-1 h-1 rounded-full bg-muted overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-1000 ${
            timeLeft <= 10
              ? "bg-red-500"
              : timeLeft <= 20
                ? "bg-orange-500"
                : "bg-primary/40"
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className={`text-xs tabular-nums ${urgentColor}`}>
        {timeLeft}s
      </span>
    </div>
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
  onContinuationUpdate,
}: ChatMessageProps) {
  // Inline tool approval handlers — visual feedback via Confirmation component
  const handleApprove = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>("approve_tool_execution", { toolId });
      if (success) {
        onApprovalUpdate?.(toolId, "approved");
      }
    } catch (error) {
      console.error("Error approving tool:", error);
    }
  }, [onApprovalUpdate]);

  const handleDeny = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>("deny_tool_execution", { toolId });
      if (success) {
        onApprovalUpdate?.(toolId, "denied");
      }
    } catch (error) {
      console.error("Error denying tool:", error);
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
        <Tool className="border-border/40">
          <ToolHeader
            type="dynamic-tool"
            state={toolState}
            toolName={msg.tool_name || "unknown"}
            title={msg.tool_name}
          />
          <ToolContent>
            {msg.tool_args && <ToolInput input={msg.tool_args} />}
            {msg.tool_id && (
              <Confirmation
                state={
                  msg.approval_state === "pending"
                    ? "approval-requested"
                    : msg.approval_state === "denied"
                      ? "output-denied"
                      : "approval-responded"
                }
              >
                <ConfirmationRequest>
                  <div className="flex flex-col gap-2 py-1">
                    {/* Risk level + target app row */}
                    <div className="flex items-center gap-2 flex-wrap">
                      {msg.risk_level && msg.risk_level !== "low" && (
                        <RiskBadge level={msg.risk_level} />
                      )}
                      {msg.target_app && (
                        <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                          <AppWindow className="size-3" />
                          {msg.target_app}
                        </span>
                      )}
                    </div>

                    <ConfirmationActions>
                      <ConfirmationAction onClick={() => handleApprove(msg.tool_id!)}>
                        Approve
                      </ConfirmationAction>
                      <ConfirmationAction variant="outline" onClick={() => handleDeny(msg.tool_id!)}>
                        Deny
                      </ConfirmationAction>
                    </ConfirmationActions>

                    <ApprovalCountdown
                      timeoutSeconds={msg.approval_timeout_seconds ?? 60}
                      isPending={msg.approval_state === "pending"}
                    />
                  </div>
                </ConfirmationRequest>
                <ConfirmationAccepted>Tool execution approved</ConfirmationAccepted>
                <ConfirmationRejected>Tool execution denied</ConfirmationRejected>
              </Confirmation>
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
        <Tool className="border-border/40">
          <ToolHeader
            type="dynamic-tool"
            state={resultState}
            toolName={msg.tool_name || "unknown"}
            title={formatAxActionTitle(msg) ?? msg.tool_name}
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
                  className="max-h-[300px] rounded-lg border border-border/30"
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
    <Message
      key={`msg-${index}-${msg.timestamp || Date.now()}`}
      from={from}
      className={from === "user" ? "max-w-[80%]" : undefined}
    >
      <MessageContent
        className={from === "user" ? "rounded-2xl" : undefined}
      >
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

        {msg.continuation_request_id && (
          <ContinuationActions
            requestId={msg.continuation_request_id}
            state={msg.continuation_state || "pending"}
            onUpdate={onContinuationUpdate}
          />
        )}

        {/* TTS spoken content — expanded when it's the only response */}
        {msg.role === "assistant" && !msg.isStreaming && (
          <TTSContentDisplay
            ttsMetadata={msg.tts_metadata}
            defaultExpanded={!msg.content || msg.content.trim() === ""}
          />
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
                className="rounded-lg w-full object-contain max-h-[300px] border border-border/30"
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
