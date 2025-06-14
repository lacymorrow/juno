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
import { Code, Copy, FileText } from "lucide-react";

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
        {msg.role === "assistant" &&
        (!msg.content || msg.content.trim() === "") ? (
          <span className="text-muted-foreground italic flex items-center gap-2">
            <span>✓</span>
            <span>Task completed successfully</span>
          </span>
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
                  onClick={() =>
                    onSaveResponse(msg.content, "markdown", index)
                  }
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