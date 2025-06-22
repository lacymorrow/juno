import { ThinkingMessage } from "@/components/ThinkingMessage";
import { ToolCallRequest, ToolCallResult } from "@/components/ToolCallMessage";
import { Button } from "@/components/ui/button";
import {
  JsxMessageRenderer,
  isJsxContent,
} from "@/components/ui/jsx-message-renderer";
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
} from "lucide-react";
import { useState } from "react";

// Type for conversation messages (imported from App.tsx)
export type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "thinking"
    | "tool_call_request"
    | "tool_call_result";
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

// Enhanced Component for displaying TTS content decoratively
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
    <div className="mb-3 pb-3 border-b border-border/30">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors group px-2 py-1.5 rounded-lg hover:bg-muted/50 w-full"
      >
        {isExpanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <Volume2 className="h-3 w-3" />
        <span className="font-medium">
          Spoken content ({ttsMetadata.tts_parts.length} part
          {ttsMetadata.tts_parts.length > 1 ? "s" : ""})
        </span>
        <span className="text-muted-foreground/60 group-hover:text-muted-foreground/80 transition-colors ml-auto">
          Click to {isExpanded ? "hide" : "show"}
        </span>
      </button>

      {isExpanded && (
        <div className="mt-3 space-y-3">
          {ttsMetadata.tts_parts.map((ttsText, index) => (
            <div
              key={index}
              className="pl-4 border-l-2 border-blue-200 dark:border-blue-800 bg-gradient-to-r from-blue-50/50 to-indigo-50/30 dark:from-blue-950/30 dark:to-indigo-950/20 rounded-r-lg p-3 backdrop-blur-sm"
            >
              <div className="flex items-center gap-2 mb-2">
                <Volume2 className="h-3 w-3 text-blue-600 dark:text-blue-400" />
                <span className="text-xs font-semibold text-blue-700 dark:text-blue-300">
                  Spoken part {index + 1}
                </span>
              </div>
              <div className="text-sm text-blue-800 dark:text-blue-200 italic leading-relaxed font-medium">
                "{ttsText.trim()}"
              </div>
            </div>
          ))}

          {ttsMetadata.tts_parts.length > 1 && (
            <div className="pl-4 border-l-2 border-green-200 dark:border-green-800 bg-gradient-to-r from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20 rounded-r-lg p-3 backdrop-blur-sm">
              <div className="flex items-center gap-2 mb-2">
                <Volume2 className="h-3 w-3 text-green-600 dark:text-green-400" />
                <span className="text-xs font-semibold text-green-700 dark:text-green-300">
                  Combined spoken text
                </span>
              </div>
              <div className="text-sm text-green-800 dark:text-green-200 italic leading-relaxed font-medium">
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
      <AIMessageContent>
        {/* TTS Content Display - Enhanced styling */}
        {msg.role === "assistant" && !msg.isStreaming && (
          <TTSContentDisplay ttsMetadata={msg.tts_metadata} />
        )}
        {msg.role === "assistant" &&
        (!msg.content || msg.content.trim() === "") ? (
          <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted/30 backdrop-blur-sm border border-border/30">
            {msg.agent_state === "Finished" ? (
              <>
                <span className="text-green-600 dark:text-green-400">✓</span>
                <span className="text-sm font-medium text-green-700 dark:text-green-300">Task completed successfully</span>
              </>
            ) : msg.agent_state === "Failed" ? (
              <>
                <span className="text-red-500">✗</span>
                <span className="text-sm font-medium text-red-600 dark:text-red-400">Task failed</span>
              </>
            ) : msg.agent_state === "Cancelled" ? (
              <>
                <span className="text-yellow-500">⊘</span>
                <span className="text-sm font-medium text-yellow-600 dark:text-yellow-400">Task cancelled</span>
              </>
            ) : msg.agent_state === "Offline" ? (
              <>
                <span className="text-orange-500">⚠</span>
                <span className="text-sm font-medium text-orange-600 dark:text-orange-400">Connection unavailable</span>
              </>
            ) : (
              <>
                <span className="text-green-600 dark:text-green-400">✓</span>
                <span className="text-sm font-medium text-green-700 dark:text-green-300">Task completed</span>
              </>
            )}
          </div>
        ) : msg.isJsx ||
          (msg.role === "assistant" &&
            !msg.isStreaming &&
            isJsxContent(msg.content)) ? (
          <JsxMessageRenderer jsx={msg.content} />
        ) : msg.role === "assistant" && msg.content ? (
          <AIResponse>{msg.content}</AIResponse>
        ) : (
          msg.content
        )}

        {/* Enhanced Screenshot Display */}
        {msg.screenshot_base64 && (
          <div className="mt-3 border-t border-border/30 pt-3">
            <div className="text-xs text-muted-foreground mb-2 font-medium">
              {msg.role === "system"
                ? "Screenshot captured by AI:"
                : "Screenshot:"}
            </div>
            <div className="relative group">
              <img
                src={`data:image/png;base64,${msg.screenshot_base64}`}
                alt="Screenshot"
                className="rounded-lg w-full object-contain max-h-[300px] border border-border/50 shadow-sm transition-all duration-200 group-hover:shadow-md"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-background/10 to-transparent pointer-events-none rounded-lg"></div>
            </div>
          </div>
        )}

        {/* Enhanced Streaming Indicator */}
        {msg.isStreaming && (
          <span className="inline-flex items-center gap-1 ml-2">
            <span className="w-1 h-4 bg-blue-500 animate-pulse rounded-full"></span>
            <span className="text-xs text-blue-600 dark:text-blue-400 font-medium">Typing...</span>
          </span>
        )}

        {/* Enhanced Action buttons for assistant messages */}
        {msg.role === "assistant" &&
          msg.content &&
          msg.content.trim() !== "" &&
          !msg.isStreaming && (
            <div className="mt-3 pt-3 border-t border-border/30 opacity-0 group-hover:opacity-100 transition-all duration-300 flex justify-end">
              <div className="flex gap-1 bg-background/95 backdrop-blur-sm rounded-lg p-1.5 shadow-sm border border-border/50">
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    "h-8 w-8 p-0 transition-all duration-200 relative rounded-lg",
                    copyingMessageId === `copy-${index}`
                      ? "bg-blue-100 text-blue-700 dark:bg-blue-900/50 dark:text-blue-300 scale-95 shadow-inner"
                      : "hover:bg-blue-50 hover:text-blue-600 dark:hover:bg-blue-950/50 dark:hover:text-blue-400 hover:scale-105 hover:shadow-sm"
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
                    "h-8 w-8 p-0 transition-all duration-200 relative rounded-lg",
                    savingMessageId === `save-html-${index}`
                      ? "bg-green-100 text-green-700 dark:bg-green-900/50 dark:text-green-300 scale-95 shadow-inner"
                      : "hover:bg-green-50 hover:text-green-600 dark:hover:bg-green-950/50 dark:hover:text-green-400 hover:scale-105 hover:shadow-sm"
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
                    "h-8 w-8 p-0 transition-all duration-200 relative rounded-lg",
                    savingMessageId === `save-markdown-${index}`
                      ? "bg-purple-100 text-purple-700 dark:bg-purple-900/50 dark:text-purple-300 scale-95 shadow-inner"
                      : "hover:bg-purple-50 hover:text-purple-600 dark:hover:bg-purple-950/50 dark:hover:text-purple-400 hover:scale-105 hover:shadow-sm"
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
