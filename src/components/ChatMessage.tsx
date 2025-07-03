import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
import { Button } from "@/components/ui/button";
import { JsxMessageRenderer } from "@/components/ui/jsx-message-renderer";
import {
  AIMessage,
  AIMessageAvatar,
  AIMessageContent,
  AIResponse,
} from "@/components/ui/kibo-ui/ai";
import { cn } from "@/lib/utils";
import {
  Code,
  Copy,
  FileText,
  Volume2,
  ChevronDown,
  ChevronRight,
  Brain,
  CheckCircle,
  XCircle,
  WifiOff,
} from "lucide-react";
import { useState } from "react";
import { UI } from "@/lib/constants.generated";

// Type for conversation messages (imported from App.tsx)
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
        className="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors group"
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
        <span className="text-muted-foreground/60 group-hover:text-muted-foreground/80 transition-colors">
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
      <div className="flex justify-start">
        <div className="flex items-start gap-3 max-w-[80%]">
          <div className="bg-gradient-to-br from-orange-500 to-red-600 text-white rounded-lg p-3 min-w-[40px] flex items-center justify-center">
            <Brain className="h-5 w-5" />
          </div>
          <div className="bg-gradient-to-br from-orange-100 to-red-100 text-orange-900 rounded-lg p-3 border border-orange-200">
            <div className="text-sm font-medium mb-1">Agent Thinking</div>
            <div className="text-sm opacity-90 whitespace-pre-wrap">
              {msg.content}
            </div>
          </div>
        </div>
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

  // Use Kibo UI components for user and assistant messages
  const from = msg.role === "user" ? "user" : "assistant";
  const avatarSrc = msg.role === "user" ? "/user-avatar.png" : "/ai-avatar.png";
  const avatarName = msg.role === "user" ? "User" : "AI";

  return (
    <AIMessage key={`msg-${index}-${msg.timestamp || Date.now()}`} from={from}>
      <AIMessageContent>
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
        ) : msg.isJsx ? (
          <JsxMessageRenderer jsx={msg.content} />
        ) : msg.role === "assistant" && msg.content ? (
          <AIResponse>{msg.content}</AIResponse>
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
              <div className="absolute inset-0 bg-gradient-to-t from-background/20 to-transparent pointer-events-none"></div>
            </div>
          </div>
        )}

        {msg.isStreaming && (
          <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse">
            |
          </span>
        )}

        {/* Action buttons for assistant messages */}
        {msg.role === "assistant" &&
          msg.content &&
          msg.content.trim() !== "" &&
          !msg.isStreaming && (
            <div className="mt-2 pt-2 border-t border-border/50 opacity-0 group-hover:opacity-100 transition-all duration-200 flex justify-end gap-2">
              <div className="flex gap-1 bg-background/90 backdrop-blur-sm rounded-md p-1 shadow-sm border">
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    "h-7 w-7 p-0 transition-all duration-150 relative",
                    copyingMessageId === `copy-${index}`
                      ? "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300 scale-95"
                      : "hover:bg-blue-50 hover:text-blue-600 dark:hover:bg-blue-950 dark:hover:text-blue-400 hover:scale-105"
                  )}
                  onClick={() => onCopyResponse(msg.content, index)}
                  disabled={copyingMessageId === `copy-${index}`}
                  title={
                    copyingMessageId === `copy-${index}`
                      ? "Copying..."
                      : "Copy response to clipboard"
                  }
                >
                  {copyingMessageId === `copy-${index}` ? (
                    <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                  ) : (
                    <Copy size={14} />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    "h-7 w-7 p-0 transition-all duration-150 relative",
                    savingMessageId === `save-html-${index}`
                      ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 scale-95"
                      : "hover:bg-green-50 hover:text-green-600 dark:hover:bg-green-950 dark:hover:text-green-400 hover:scale-105"
                  )}
                  onClick={() => onSaveResponse(msg.content, "html", index)}
                  disabled={savingMessageId === `save-html-${index}`}
                  title={
                    savingMessageId === `save-html-${index}`
                      ? "Saving HTML..."
                      : "Save as HTML file with professional styling"
                  }
                >
                  {savingMessageId === `save-html-${index}` ? (
                    <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                  ) : (
                    <Code size={14} />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    "h-7 w-7 p-0 transition-all duration-150 relative",
                    savingMessageId === `save-markdown-${index}`
                      ? "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300 scale-95"
                      : "hover:bg-purple-50 hover:text-purple-600 dark:hover:bg-purple-950 dark:hover:text-purple-400 hover:scale-105"
                  )}
                  onClick={() => onSaveResponse(msg.content, "markdown", index)}
                  disabled={savingMessageId === `save-markdown-${index}`}
                  title={
                    savingMessageId === `save-markdown-${index}`
                      ? "Saving Markdown..."
                      : "Save as Markdown file for documentation"
                  }
                >
                  {savingMessageId === `save-markdown-${index}` ? (
                    <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                  ) : (
                    <FileText size={14} />
                  )}
                </Button>
              </div>
            </div>
          )}
      </AIMessageContent>
      <AIMessageAvatar src={avatarSrc} name={avatarName} />
    </AIMessage>
  );
}
