import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
import {
  MixedContentRenderer,
  hasMixedContent,
} from "@/components/ui/mixed-content-renderer";
import {
  Message,
  MessageContent,
  MessageActions,
  MessageAction,
  MessageToolbar,
} from "@/components/ui/message";
import { Response } from "@/components/ui/response";
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

// Component for displaying TTS content decoratively
function TTSContentDisplay({
  ttsMetadata,
}: {
  ttsMetadata: ChatMessage["tts_metadata"];
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  if (!ttsMetadata?.has_spoken_content || !ttsMetadata.tts_parts.length) {
    return null;
  }

  return (
    <div className="mb-2 pb-2 border-b border-border/30">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors group/tts"
      >
        {isExpanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <Volume2 className="h-3 w-3" />
        <span>
          Spoken content ({ttsMetadata.tts_parts.length} part
          {ttsMetadata.tts_parts.length > 1 ? "s" : ""})
        </span>
        <span className="text-muted-foreground/60 group-hover/tts:text-muted-foreground/80 transition-colors">
          Click to {isExpanded ? "hide" : "show"}
        </span>
      </button>

      {isExpanded && (
        <div className="mt-2 space-y-2">
          {ttsMetadata.tts_parts.map((ttsText, index) => (
            <div
              key={index}
              className="pl-4 border-l-2 border-blue-200 dark:border-blue-800 bg-blue-50/30 dark:bg-blue-900/10 rounded-r-md p-2"
            >
              <div className="flex items-center gap-2 mb-1">
                <Volume2 className="h-3 w-3 text-blue-600 dark:text-blue-400" />
                <span className="text-xs font-medium text-blue-700 dark:text-blue-300">
                  Spoken part {index + 1}
                </span>
              </div>
              <div className="text-sm text-blue-800 dark:text-blue-200 italic leading-relaxed">
                "{ttsText.trim()}"
              </div>
            </div>
          ))}

          {ttsMetadata.tts_parts.length > 1 && (
            <div className="pl-4 border-l-2 border-green-200 dark:border-green-800 bg-green-50/30 dark:bg-green-900/10 rounded-r-md p-2">
              <div className="flex items-center gap-2 mb-1">
                <Volume2 className="h-3 w-3 text-green-600 dark:text-green-400" />
                <span className="text-xs font-medium text-green-700 dark:text-green-300">
                  Combined spoken text
                </span>
              </div>
              <div className="text-sm text-green-800 dark:text-green-200 italic leading-relaxed">
                "{ttsMetadata.total_spoken_text.trim()}"
              </div>
            </div>
          )}
        </div>
      )}
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
        {/* TTS Content Display - Show decoratively */}
        {msg.role === "assistant" && !msg.isStreaming && (
          <TTSContentDisplay ttsMetadata={msg.tts_metadata} />
        )}
        {msg.role === "assistant" &&
        (!msg.content || msg.content.trim() === "") ? (
          <span className="text-muted-foreground italic flex items-center gap-2">
            {msg.agent_state === UI.AGENT_STATUS_FINISHED ? (
              <div className="flex items-center gap-1.5 text-green-600">
                <CheckCircle className="h-3 w-3" />
                <span className="text-xs">Complete</span>
              </div>
            ) : msg.agent_state === UI.AGENT_STATUS_FAILED ? (
              <div className="flex items-center gap-1.5 text-red-600">
                <XCircle className="h-3 w-3" />
                <span className="text-xs">Failed</span>
              </div>
            ) : msg.agent_state === UI.AGENT_STATUS_CANCELLED ? (
              <div className="flex items-center gap-1.5 text-gray-600">
                <XCircle className="h-3 w-3" />
                <span className="text-xs">Cancelled</span>
              </div>
            ) : msg.agent_state === UI.AGENT_STATUS_OFFLINE ? (
              <div className="flex items-center gap-1.5 text-gray-600">
                <WifiOff className="h-3 w-3" />
                <span className="text-xs">Offline</span>
              </div>
            ) : (
              <div className="flex items-center gap-1.5 text-green-600">
                <CheckCircle className="h-3 w-3" />
                <span className="text-xs">Complete</span>
              </div>
            )}
          </span>
        ) : msg.role === "assistant" && msg.content && (msg.isJsx || hasMixedContent(msg.content)) ? (
          <MixedContentRenderer content={msg.content} isStreaming={msg.isStreaming} />
        ) : msg.role === "assistant" && msg.content ? (
          <Response>{msg.content}</Response>
        ) : msg.role === "user" ? (
          <span>{msg.content}</span>
        ) : (
          msg.content
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
