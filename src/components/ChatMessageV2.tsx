import {
  MixedContentRenderer,
  splitMixedContent,
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
import { Button } from "@/components/ui/button";
import {
  Check,
  Copy,
  Share,
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
  Image as ImageIcon,
  ImageOff,
} from "lucide-react";
import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { UI, COMMANDS } from "@/lib/constants.generated";

export type { ChatMessage } from "@/types/chat";
import type { ChatMessage, ResponseExportInput } from "@/types/chat";
import type { ShareAnchor } from "@/hooks/useConversation";
import type { TurnScreenshot } from "@/lib/turn-summary";

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
  /** The user message this reply answers; titles the exported document. */
  question?: string;
  copiedMessageId: string | null;
  onCopyResponse: (response: ResponseExportInput, index: number) => void;
  onShareResponse: (response: ResponseExportInput, anchor: ShareAnchor) => void;
  onApprovalUpdate?: (toolId: string, state: "approved" | "denied") => void;
  onContinuationUpdate?: (requestId: string, state: "stopped" | "continued") => void;
  /**
   * Whether to draw the tool cards: the tool name, its JSON arguments and its
   * output. Development only. Production keeps the approval question and drops
   * the rest, so the default here is the production answer.
   */
  showToolDetails?: boolean;
  /**
   * Every capture this turn made, for an assistant reply.
   *
   * The images live on the tool rows, which the transcript does not draw, so
   * the reply has to be handed them. The container computes the list because
   * only it can see the turn; this component only draws it.
   */
  screenshots?: TurnScreenshot[];
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
        type="button"
        aria-expanded={isExpanded}
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

/** A capture that cannot be drawn, said in words instead of a broken image. */
function CaptureProblem({ text }: { text: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground/70">
      <ImageOff className="h-3 w-3 shrink-0" />
      <span>{text}</span>
    </span>
  );
}

/**
 * What the agent saw, behind one disclosure.
 *
 * Sibling of TTSContentDisplay on purpose. A turn has two side channels — what
 * it said aloud and what it looked at — and they read as one family of compact
 * rows rather than two inventions. Collapsed by default, because a computer-use
 * turn takes five or six full-screen captures and a wall of retina PNGs buries
 * the answer the person actually asked for.
 *
 * Collapsed means unmounted, not hidden: the `isExpanded &&` below removes the
 * `<img>` elements so the browser can release the decoded bitmaps. The base64
 * strings sit in the conversation either way, so this decides what is decoded,
 * never what is retained.
 */
function ScreenshotDisclosure({
  screenshots,
}: {
  screenshots?: TurnScreenshot[];
}) {
  const [isExpanded, setIsExpanded] = useState(false);
  // Which images the browser refused, by position. A data URI that is truncated
  // or not actually a PNG fires `onError` and would otherwise leave the broken
  // image glyph sitting in the transcript.
  const [brokenAt, setBrokenAt] = useState<Record<number, true>>({});

  if (!screenshots?.length) return null;

  const shown = screenshots.filter((shot) => shot.base64);
  const missing = screenshots.length - shown.length;

  // Nothing came back at all. There is nothing to open, so this is a plain
  // status line — an icon and words, no chevron — and the label never claims
  // Juno saw a screen it never got.
  if (shown.length === 0) {
    return (
      <div className="mt-1">
        <CaptureProblem
          text={
            missing === 1
              ? "A screen capture did not come back."
              : `${missing} screen captures did not come back.`
          }
        />
      </div>
    );
  }

  // Counts what there is to look at, so the label and the stack agree. A later
  // `onError` is reported per image rather than by rewriting this line, which
  // would change the row's text under the reader's cursor.
  const label =
    shown.length === 1 ? "Saw the screen" : `Saw the screen ${shown.length} times`;

  return (
    <div className="mt-1">
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded(!isExpanded)}
        className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground/70 hover:text-muted-foreground transition-colors"
      >
        {isExpanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <ImageIcon className="h-3 w-3" />
        <span>{label}</span>
      </button>

      {isExpanded && (
        // A stack in capture order, not a strip. Several retina screens side by
        // side in a 360px pane are too small to read, and the order is the story
        // of the turn.
        <div className="mt-1.5 flex flex-col gap-1.5 pl-5">
          {shown.map((shot, i) =>
            brokenAt[i] ? (
              <CaptureProblem
                key={i}
                text="This capture could not be shown."
              />
            ) : (
              <img
                key={i}
                src={`data:image/png;base64,${shot.base64}`}
                alt={
                  shown.length > 1
                    ? `Screenshot ${i + 1} of ${shown.length}${shot.toolName ? ` from ${shot.toolName}` : ""}`
                    : `Screenshot${shot.toolName ? ` from ${shot.toolName}` : ""}`
                }
                onError={() => setBrokenAt((prev) => ({ ...prev, [i]: true }))}
                className="w-full max-h-[320px] rounded-md border border-border/40 object-contain"
              />
            )
          )}
          {missing > 0 && (
            <CaptureProblem
              text={
                missing === 1
                  ? "One more capture did not come back."
                  : `${missing} more captures did not come back.`
              }
            />
          )}
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
      await invoke(COMMANDS.AGENT_RESPOND_TO_AGENT_CONTINUATION, {
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
      await invoke(COMMANDS.AGENT_RESPOND_TO_AGENT_CONTINUATION, {
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
  { label: string; className: string; Icon: React.ComponentType<{ className?: string }> }
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
  variant = "bar",
}: {
  timeoutSeconds: number;
  isPending: boolean;
  /**
   * `bar` is the development card's progress meter. `text` is one calm line,
   * for the question a person reads in production: the deadline is real, so it
   * is said, but a reddening bar is alarm for its own sake.
   */
  variant?: "bar" | "text";
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

  if (variant === "text") {
    return (
      <p className="mt-2 text-xs text-muted-foreground/70 tabular-nums">
        Expires in {timeLeft}s
      </p>
    );
  }

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

/**
 * A tool call the person still has to answer.
 *
 * `approval_state` is set only by the backend's approval request, so it is what
 * separates "Juno is asking" from "Juno is working". Production hides the
 * working rows and keeps the asking ones: a risky action that cannot ask is an
 * agent that blocks forever on an answer nobody can give.
 */
export function awaitsApproval(msg: ChatMessage): boolean {
  return msg.role === "tool_call_request" && msg.approval_state !== undefined;
}

/** The debug rows: a tool call, or a result that arrived without its call. */
export function isToolRow(msg: ChatMessage): boolean {
  return msg.role === "tool_call_request" || msg.role === "tool_call_result";
}

/**
 * A tool approval with the tool plumbing taken out.
 *
 * Production never shows tool rows, but the approval is not a tool row, it is a
 * question. So it loses the tool card, the tool name badge, the risk level and
 * the raw JSON arguments, and keeps the one sentence the backend already wrote,
 * which app it touches, and two buttons.
 *
 * It sits at the top level of the transcript on purpose. Nested inside the tool
 * disclosure, which is where it lives in development, the question is one click
 * away from being missed, and a question a person does not see is an agent that
 * waits forever.
 */
function ApprovalPrompt({
  msg,
  onApprove,
  onDeny,
  onAlwaysAllow,
  grantedLabel,
}: {
  msg: ChatMessage;
  onApprove: (toolId: string) => void;
  onDeny: (toolId: string) => void;
  /** Answer yes, and stop being asked about this kind of action. */
  onAlwaysAllow: (toolId: string, label: string) => void;
  /**
   * Set once this row was answered with the standing yes, so the settled line
   * can say what that yes covers. A grant a person cannot see they gave is the
   * thing this feature was careful not to build.
   */
  grantedLabel: string | null;
}) {
  const toolId = msg.tool_id;
  const detail = msg.content?.trim();

  if (msg.approval_state === "approved") {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <CheckCircle className="h-3 w-3 text-muted-foreground/50" />
        <span>
          {grantedLabel
            ? `Allowed. Juno will not ask about ${grantedLabel} again in this conversation.`
            : `Allowed${detail ? `: ${detail}` : ""}`}
        </span>
      </span>
    );
  }

  if (msg.approval_state === "denied") {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground italic">
        <XCircle className="h-3 w-3 text-muted-foreground/50" />
        <span>Not allowed{detail ? `: ${detail}` : ""}</span>
      </span>
    );
  }

  return (
    <div className="w-full rounded-lg border border-border bg-muted/30 px-3 py-2.5">
      <p className="text-sm font-medium text-foreground">Juno needs your OK</p>
      {detail && (
        <p className="mt-1 text-sm text-muted-foreground break-words">{detail}</p>
      )}

      {/* Which app it touches, because that is the part a person can picture.
          No risk level: "Critical" in red tells someone who cannot act on it
          only that they should be frightened. */}
      {msg.target_app && (
        <p className="mt-2 inline-flex items-center gap-1 text-xs text-muted-foreground">
          <AppWindow className="size-3" />
          {msg.target_app}
        </p>
      )}

      {toolId && (
        <div className="mt-2.5 flex flex-col items-start gap-2">
          <div className="flex items-center gap-2">
            <Button size="sm" onClick={() => onApprove(toolId)}>
              Allow
            </Button>
            <Button size="sm" variant="outline" onClick={() => onDeny(toolId)}>
              Don't allow
            </Button>
          </div>

          {/* The third answer, not a setting.
              It is phrased as an answer and it begins with the same word as the
              first one, because that is what it is: yes, and keep saying yes to
              this kind of thing. A checkbox beside Allow would have turned a
              question into a preferences panel, and the whole complaint was
              that permissions felt like a configuration screen.
              It is quiet on purpose: Allow stays the one primary action.
              Rust leaves `always_allow_label` out for anything irreversible, so
              this answer is absent exactly where it could not be honoured. */}
          {msg.always_allow_label && (
            <button
              type="button"
              onClick={() => onAlwaysAllow(toolId, msg.always_allow_label!)}
              className="text-left text-xs text-[#007AFF] underline-offset-2 hover:underline"
            >
              Allow, and stop asking about {msg.always_allow_label}
              <span className="text-muted-foreground"> for this conversation</span>
            </button>
          )}
        </div>
      )}

      <ApprovalCountdown
        timeoutSeconds={msg.approval_timeout_seconds ?? 60}
        isPending={msg.approval_state === "pending"}
        variant="text"
      />
    </div>
  );
}

/** Spoken markers never reach the page, but export.rs documents content that still carries them. */
export const TTS_BLOCK = /<TTS>[\s\S]*?<\/TTS>/g;

/**
 * Whether this message is worth offering to copy or share.
 *
 * Copy and Share export a reply as a document, so they only earn their place
 * when there is a reply to export. Three things that look like replies are not:
 * a notice the app wrote about itself during onboarding, a turn that spoke its
 * answer and left nothing on screen, and a run whose only visible output is a
 * collapsed "Why I did it this way". Offering to export those is how the
 * toolbar ended up on almost every row.
 */
export function isWorthExporting(msg: ChatMessage): boolean {
  if (msg.role !== "assistant" || msg.isStreaming || msg.notice) return false;
  const content = msg.content?.replace(TTS_BLOCK, "").trim();
  if (!content) return false;
  // Prose and generated components both carry words into the export; a why
  // block on its own does not.
  return splitMixedContent(content, false).some(
    (segment) =>
      (segment.type === "text" || segment.type === "jsx") &&
      segment.content.trim() !== ""
  );
}

export function ChatMessageComponent({
  msg,
  index,
  question,
  copiedMessageId,
  onCopyResponse,
  onShareResponse,
  onApprovalUpdate,
  onContinuationUpdate,
  showToolDetails = false,
  screenshots,
}: ChatMessageProps) {
  const copied = copiedMessageId === `copy-${index}`;
  const exportInput = (): ResponseExportInput => ({
    question,
    content: msg.content,
    spoken: msg.tts_metadata?.tts_parts ?? [],
    timestamp: msg.timestamp,
  });

  /** Which standing yes this row was answered with, if it was. Display only. */
  const [grantedLabel, setGrantedLabel] = useState<string | null>(null);

  // Inline tool approval handlers — visual feedback via Confirmation component
  const handleApprove = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>(COMMANDS.TOOLS_APPROVE_TOOL_EXECUTION, { toolId });
      if (success) {
        onApprovalUpdate?.(toolId, "approved");
      }
    } catch (error) {
      console.error("Error approving tool:", error);
    }
  }, [onApprovalUpdate]);

  const handleDeny = useCallback(async (toolId: string) => {
    try {
      const success = await invoke<boolean>(COMMANDS.TOOLS_DENY_TOOL_EXECUTION, { toolId });
      if (success) {
        onApprovalUpdate?.(toolId, "denied");
      }
    } catch (error) {
      console.error("Error denying tool:", error);
    }
  }, [onApprovalUpdate]);

  /**
   * Allow this, and stop asking about the same kind of action for the rest of
   * this conversation.
   *
   * Scope is one tool, one conversation, memory only. Rust forgets every grant
   * when Juno quits and when the permission mode changes, which is why there is
   * no list of grants to review: nothing outlives the session that would need
   * one. Irreversible actions are never granted, and Rust leaves
   * `always_allow_label` out for them so this answer is not even offered.
   */
  const handleAlwaysAllow = useCallback(async (toolId: string, label: string) => {
    try {
      const success = await invoke<boolean>(COMMANDS.TOOLS_ALLOW_TOOL_FOR_CONVERSATION, { toolId });
      if (success) {
        // Purely visual, and purely local: Rust holds the grant, this only
        // remembers which of the three answers was pressed so the settled line
        // can name what the person just agreed to. `approval_state` cannot
        // carry it, and widening the backend's event to say "approved, and
        // standing" would be a protocol change for one sentence of copy.
        setGrantedLabel(label);
        onApprovalUpdate?.(toolId, "approved");
      }
    } catch (error) {
      console.error("Error allowing the tool for this conversation:", error);
    }
  }, [onApprovalUpdate]);

  // Thinking messages: collapsed, and they stay collapsed.
  //
  // `defaultOpen={false}` is the Reasoning component's explicit-closed path: it
  // suppresses the auto-open that used to throw the panel open on every
  // thinking block and close it a second later, which moved the whole
  // transcript twice per turn. The duration effect is independent of the open
  // state, so the collapsed row still reports "Thought for 3 seconds"; opening
  // it is the person's call.
  if (msg.role === "thinking") {
    return (
      <div className="flex justify-start w-full">
        <Reasoning defaultOpen={false} isStreaming={msg.isStreaming}>
          <ReasoningTrigger />
          <ReasoningContent>{msg.content}</ReasoningContent>
        </Reasoning>
      </div>
    );
  }

  // Tool call requests — with inline approval if pending
  if (msg.role === "tool_call_request") {
    // Outside development a tool call is not shown at all, with one exception:
    // one Juno is waiting on. ChatContainerV2 filters the rest out of the list
    // before they reach here, so this branch only has the question to draw.
    if (!showToolDetails) {
      if (!awaitsApproval(msg)) return null;
      return (
        <div className="flex justify-start w-full">
          <ApprovalPrompt
            msg={msg}
            onApprove={handleApprove}
            onDeny={handleDeny}
            onAlwaysAllow={handleAlwaysAllow}
            grantedLabel={grantedLabel}
          />
        </div>
      );
    }

    // One row per tool call. `success` is undefined until the result folds in,
    // so the same message carries the call through running -> done/failed
    // instead of spawning a second row underneath it.
    const hasResult = msg.success !== undefined;
    const toolState = msg.approval_state === "pending"
      ? "approval-requested" as const
      : msg.approval_state === "denied"
        ? "output-denied" as const
        : hasResult
          ? (msg.success ? "output-available" as const : "output-error" as const)
          : "input-available" as const;

    return (
      <div className="flex justify-start w-full">
        <Tool className="border-border/40">
          <ToolHeader
            type="dynamic-tool"
            state={toolState}
            toolName={msg.tool_name || "unknown"}
            title={formatAxActionTitle(msg) ?? msg.tool_name}
          />
          <ToolContent>
            {msg.tool_args && <ToolInput input={msg.tool_args} />}
            {hasResult && (
              <ToolOutput
                output={msg.tool_output}
                errorText={msg.success ? undefined : msg.result_content}
              />
            )}
            <ScreenshotDisclosure
              screenshots={
                msg.screenshot_base64
                  ? [{ base64: msg.screenshot_base64, toolName: msg.tool_name }]
                  : undefined
              }
            />
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
                      {/* No standing-yes answer here on purpose. This is the
                          development card, which #644 deliberately left as it
                          was, and the third answer belongs with the question a
                          person actually reads, not in a debug disclosure. */}
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
    // A result that arrived without its call. Nothing to ask, so production
    // shows nothing; the container has already filtered it out.
    if (!showToolDetails) return null;

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
            <ScreenshotDisclosure
              screenshots={
                msg.screenshot_base64
                  ? [{ base64: msg.screenshot_base64, toolName: msg.tool_name }]
                  : undefined
              }
            />
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
          // A stream that has not produced text yet is not "Done": only the
          // shimmer below shows until the first chunk or the end of the stream.
          msg.isStreaming ? null : <AgentStatusBadge agentState={msg.agent_state} />
        ) : msg.role === "assistant" && msg.content && msg.isJsx ? (
          <MixedContentRenderer content={msg.content} isStreaming={msg.isStreaming} />
        ) : msg.role === "assistant" && msg.content ? (
          <MessageResponse>{msg.content}</MessageResponse>
        ) : msg.role === "user" ? (
          <>
            {/* What they attached, above what they typed — the bubble shows
                the whole message, so a pasted photo is visibly on its way
                rather than invisible to the person who sent it. */}
            {msg.images && msg.images.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {msg.images.map((src, i) => (
                  <img
                    key={`${msg.messageId || index}-img-${i}`}
                    src={src}
                    alt="Attached image"
                    className="max-h-32 max-w-48 rounded-lg object-cover"
                  />
                ))}
              </div>
            )}
            {msg.content && <span>{msg.content}</span>}
          </>
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
        {/*
          Shown whenever there is spoken content, streaming or not. Gating this
          on `!isStreaming` meant a turn that kept going — consecutive tool
          calls, an agent loop — never showed what it had already said aloud,
          because the bubble stays open for the whole turn. TTSContentDisplay
          renders nothing when there is no spoken content, so this is safe.
        */}
        {msg.role === "assistant" && (
          <TTSContentDisplay
            ttsMetadata={msg.tts_metadata}
            defaultExpanded={
              !msg.isStreaming && (!msg.content || msg.content.trim() === "")
            }
          />
        )}

        {/* What this turn saw, next to what it said. The two rows are the same
            shape on purpose; see ScreenshotDisclosure.

            This used to read `msg.screenshot_base64` straight off the bubble,
            which was never set on an assistant or system message: Rust's
            SubmitQueryResult carries `screenshot_data`, always None. The images
            ride on the tool rows the transcript hides, so the container gathers
            the turn's captures and hands them down. */}
        {msg.role === "assistant" && (
          <ScreenshotDisclosure screenshots={screenshots} />
        )}

        {msg.isStreaming && (
          <Shimmer as="span" duration={1.5}>...</Shimmer>
        )}
      </MessageContent>

      {/* Action toolbar, only where there is something worth keeping */}
      {isWorthExporting(msg) && (
          <MessageToolbar className="opacity-40 transition-opacity duration-200 group-hover:opacity-100 focus-within:opacity-100">
            <MessageActions>
              <MessageAction
                tooltip={copied ? "Copied" : "Copy"}
                onClick={() => onCopyResponse(exportInput(), index)}
                className="h-7 w-7"
              >
                {copied ? (
                  <Check size={14} />
                ) : (
                  <Copy size={14} />
                )}
              </MessageAction>
              <MessageAction
                tooltip="Share"
                onClick={(event) => {
                  const box = event.currentTarget.getBoundingClientRect();
                  onShareResponse(exportInput(), {
                    x: box.left,
                    y: box.top,
                    width: box.width,
                    height: box.height,
                  });
                }}
                className="h-7 w-7"
              >
                <Share size={14} />
              </MessageAction>
            </MessageActions>
          </MessageToolbar>
        )}
    </Message>
  );
}
