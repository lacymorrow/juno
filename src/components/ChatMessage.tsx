import React from "react";
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
import { ChatMessage as ChatMessageType } from "@/types/app.types";
import { Code, Copy, FileText } from "lucide-react";

interface ChatMessageProps {
  message: ChatMessageType;
  index: number;
  copyingMessageId: string | null;
  savingMessageId: string | null;
  onCopyResponse: (content: string, index: number) => void;
  onSaveResponse: (content: string, format: "html" | "markdown", index: number) => void;
}

export const ChatMessage = ({
  message,
  index,
  copyingMessageId,
  savingMessageId,
  onCopyResponse,
  onSaveResponse,
}: ChatMessageProps) => {
  // Handle special message types with existing components
  if (message.role === "thinking") {
    return (
      <div
        key={`msg-${index}-${message.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ThinkingMessage content={message.content} timestamp={message.timestamp} />
      </div>
    );
  }

  if (message.role === "tool_call_request") {
    return (
      <div
        key={`msg-${index}-${message.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ToolCallRequest
          toolName={message.tool_name || "unknown"}
          toolArgs={message.tool_args}
          content={message.content}
          timestamp={message.timestamp}
        />
      </div>
    );
  }

  if (message.role === "tool_call_result") {
    return (
      <div
        key={`msg-${index}-${message.timestamp || Date.now()}`}
        className="flex justify-start"
      >
        <ToolCallResult
          toolName={message.tool_name || "unknown"}
          toolOutput={message.tool_output}
          success={message.success ?? true}
          content={message.content}
          screenshot_base64={message.screenshot_base64}
          timestamp={message.timestamp}
        />
      </div>
    );
  }

  // Use Kibo UI components for user and assistant messages
  const from = message.role === "user" ? "user" : "assistant";
  const avatarSrc = message.role === "user" ? "/user-avatar.png" : "/ai-avatar.png";
  const avatarName = message.role === "user" ? "User" : "AI";

  return (
    <AIMessage key={`msg-${index}-${message.timestamp || Date.now()}`} from={from}>
      <AIMessageContent>
        {message.role === "assistant" &&
        (!message.content || message.content.trim() === "") ? (
          <span className="text-muted-foreground italic flex items-center gap-2">
            <span>✓</span>
            <span>Task completed successfully</span>
          </span>
        ) : message.isJsx ||
          (message.role === "assistant" &&
            !message.isStreaming &&
            isJsxContent(message.content)) ? (
          <JsxMessageRenderer jsx={message.content} />
        ) : message.role === "assistant" && message.content ? (
          <AIResponse>{message.content}</AIResponse>
        ) : (
          message.content
        )}

        {message.screenshot_base64 && (
          <div className="mt-2 border-t pt-2">
            <div className="text-xs text-muted-foreground mb-1">
              {message.role === "system"
                ? "Screenshot captured by AI:"
                : "Screenshot:"}
            </div>
            <div className="relative">
              <img
                src={`data:image/png;base64,${message.screenshot_base64}`}
                alt="Screenshot"
                className="rounded w-full object-contain max-h-[300px] border border-border shadow-sm"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-background/20 to-transparent pointer-events-none"></div>
            </div>
          </div>
        )}

        {message.isStreaming && (
          <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse">
            |
          </span>
        )}

        {/* Action buttons for assistant messages */}
        {message.role === "assistant" &&
          message.content &&
          message.content.trim() !== "" &&
          !message.isStreaming && (
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
                  onClick={() => onCopyResponse(message.content, index)}
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
                  onClick={() => onSaveResponse(message.content, "html", index)}
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
                  onClick={() => onSaveResponse(message.content, "markdown", index)}
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
};