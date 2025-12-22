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
  CheckCircle,
  XCircle,
  WifiOff,
} from "lucide-react";
import { useState } from "react";
import { UI } from "@/lib/constants.generated";
import { ThinkingMessage } from "./ThinkingMessage";

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
    <div className="mb-2 pb-2 border-b border-border/10">
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
              className="pl-4 border-l-2 border-blue-200/50 dark:border-blue-800/50 bg-blue-50/10 dark:bg-blue-900/10 rounded-r-md p-2"
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
            <div className="pl-4 border-l-2 border-green-200/50 dark:border-green-800/50 bg-green-50/10 dark:bg-green-900/10 rounded-r-md p-2">
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
        <ThinkingMessage content={msg.content} timestamp={msg.timestamp} />
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
      <AIMessageContent
        className={cn(
          "relative overflow-hidden transition-all duration-300",
          msg.role === "user"
            ? "bg-gradient-to-br from-blue-500 to-blue-600 text-white border-0 shadow-md"
            : "bg-background/40 backdrop-blur-md border border-white/10 dark:border-white/5 shadow-sm hover:border-white/20 dark:hover:border-white/10",
          msg.role === "assistant" && "bg-zinc-50/50 dark:bg-zinc-900/50"
        )}
      >
        {/* Subtle gradient overlay for AI messages */}
        {msg.role === "assistant" && (
          <div className="absolute inset-0 bg-gradient-to-br from-white/40 via-transparent to-transparent dark:from-white/5 pointer-events-none opacity-50" />
        )}
        
        <div className="relative z-10">
          {/* TTS Content Display - Show decoratively */}
          {msg.role === "assistant" && !msg.isStreaming && (
            <TTSContentDisplay ttsMetadata={msg.tts_metadata} />
          )}
          {msg.role === "assistant" &&
          (!msg.content || msg.content.trim() === "") ? (
            <span className="text-muted-foreground italic flex items-center gap-2">
              {msg.agent_state === UI.AGENT_STATUS_FINISHED ? (
                <div className="flex items-center gap-1.5 text-green-600 dark:text-green-400">
                  <CheckCircle className="h-3 w-3" />
                  <span className="text-xs font-medium">Complete</span>
                </div>
              ) : msg.agent_state === UI.AGENT_STATUS_FAILED ? (
                <div className="flex items-center gap-1.5 text-red-600 dark:text-red-400">
                  <XCircle className="h-3 w-3" />
                  <span className="text-xs font-medium">Failed</span>
                </div>
              ) : msg.agent_state === UI.AGENT_STATUS_CANCELLED ? (
                <div className="flex items-center gap-1.5 text-gray-600 dark:text-gray-400">
                  <XCircle className="h-3 w-3" />
                  <span className="text-xs font-medium">Cancelled</span>
                </div>
              ) : msg.agent_state === UI.AGENT_STATUS_OFFLINE ? (
                <div className="flex items-center gap-1.5 text-gray-600 dark:text-gray-400">
                  <WifiOff className="h-3 w-3" />
                  <span className="text-xs font-medium">Offline</span>
                </div>
              ) : (
                <div className="flex items-center gap-1.5 text-green-600 dark:text-green-400">
                  <CheckCircle className="h-3 w-3" />
                  <span className="text-xs font-medium">Complete</span>
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
            <div className="mt-3 border-t border-border/20 pt-3">
              <div className="text-xs text-muted-foreground mb-2 flex items-center gap-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-blue-500/50" />
                {msg.role === "system"
                  ? "Screenshot captured by AI"
                  : "Attached Screenshot"}
              </div>
              <div className="relative group overflow-hidden rounded-md border border-border/20 shadow-sm transition-all hover:shadow-md">
                <img
                  src={`data:image/png;base64,${msg.screenshot_base64}`}
                  alt="Screenshot"
                  className="w-full object-contain max-h-[300px] bg-black/5 dark:bg-white/5 transition-transform duration-500 group-hover:scale-[1.02]"
                />
                <div className="absolute inset-0 bg-gradient-to-t from-black/20 to-transparent pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity duration-300"></div>
              </div>
            </div>
          )}

          {msg.isStreaming && (
            <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse rounded-sm opacity-70">
              |
            </span>
          )}

          {/* Action buttons for assistant messages */}
          {msg.role === "assistant" &&
            msg.content &&
            msg.content.trim() !== "" &&
            !msg.isStreaming && (
              <div className="mt-2 pt-2 border-t border-border/10 opacity-0 group-hover:opacity-100 transition-all duration-200 flex justify-end gap-1">
                <div className="flex gap-1 bg-background/50 backdrop-blur-sm rounded-lg p-0.5 shadow-sm border border-border/10">
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      "h-7 w-7 p-0 transition-all duration-200 relative rounded-md",
                      copyingMessageId === `copy-${index}`
                        ? "bg-blue-100/20 text-blue-600 dark:text-blue-400 scale-95"
                        : "hover:bg-blue-50/20 hover:text-blue-600 dark:hover:text-blue-400 hover:scale-105 text-muted-foreground"
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
                      <Copy size={13} strokeWidth={2} />
                    )}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      "h-7 w-7 p-0 transition-all duration-200 relative rounded-md",
                      savingMessageId === `save-html-${index}`
                        ? "bg-green-100/20 text-green-600 dark:text-green-400 scale-95"
                        : "hover:bg-green-50/20 hover:text-green-600 dark:hover:text-green-400 hover:scale-105 text-muted-foreground"
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
                      <Code size={13} strokeWidth={2} />
                    )}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      "h-7 w-7 p-0 transition-all duration-200 relative rounded-md",
                      savingMessageId === `save-markdown-${index}`
                        ? "bg-purple-100/20 text-purple-600 dark:text-purple-400 scale-95"
                        : "hover:bg-purple-50/20 hover:text-purple-600 dark:hover:text-purple-400 hover:scale-105 text-muted-foreground"
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
                      <FileText size={13} strokeWidth={2} />
                    )}
                  </Button>
                </div>
              </div>
            )}
        </div>
      </AIMessageContent>
      <AIMessageAvatar src={avatarSrc} name={avatarName} className="ring-2 ring-background shadow-md" />
    </AIMessage>
  );
}
