import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
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
import { useState } from "react";
import { UI } from "@/lib/constants.generated";
import { ThinkingMessage } from "./ThinkingMessage";

// Type for conversation messages — same as original ChatMessage
export type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "tool_call_request"
    | "tool_call_result"
    | "thinking";
  content: string;
  isJsx?: boolean;
  screenshot_base64?: string;
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean;
  timestamp?: number;
  isStreaming?: boolean;
  messageId?: string;
  agent_state?: string;
  tts_metadata?: {
    has_spoken_content: boolean;
    tts_parts: string[];
    total_spoken_text: string;
  };
};

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
}: ChatMessageProps) {
  // Handle special message types with existing components
  if (msg.role === "thinking") {
    return (
      <div
        key={`msg-${index}-${msg.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ThinkingMessage content={msg.content} timestamp={msg.timestamp} isStreaming={msg.isStreaming} />
      </div>
    );
  }

  if (msg.role === "tool_call_request") {
    return (
      <div
        key={`msg-${index}-${msg.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ToolCallRequest
          toolName={msg.tool_name || "unknown"}
          toolArgs={msg.tool_args}
          content={msg.content}
          timestamp={msg.timestamp}
        />
      </div>
    );
  }

  if (msg.role === "tool_call_result") {
    return (
      <div
        key={`msg-${index}-${msg.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ToolCallResult
          toolName={msg.tool_name || "unknown"}
          toolOutput={msg.tool_output}
          success={msg.success ?? true}
          content={msg.content}
          screenshot_base64={msg.screenshot_base64}
          timestamp={msg.timestamp}
        />
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
          <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse">
            |
          </span>
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
