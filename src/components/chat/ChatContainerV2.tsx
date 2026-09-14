import React from "react";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ai-elements/conversation";
import { ChatMessageComponent } from "@/components/ChatMessageV2";
import type { ChatMessage, ResponseExportInput } from "@/types/chat";
import type { ShareAnchor } from "@/hooks/useConversation";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import { cn } from "@/lib/utils";

// Helper function to determine if timestamp should be shown (similar to Slack/Apple Messages)
function shouldShowTimestamp(
  currentMessage: ChatMessage,
  previousMessage: ChatMessage | null,
  timeThresholdMinutes: number = 5
): boolean {
  if (!currentMessage.timestamp) return false;
  if (!previousMessage || !previousMessage.timestamp) return true;

  const timeDiffMinutes =
    (currentMessage.timestamp - previousMessage.timestamp) / (1000 * 60);
  return timeDiffMinutes >= timeThresholdMinutes;
}

// Helper function to format timestamp for display
function formatMessageTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  if (isToday) {
    return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } else {
    return date.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }
}

// Helper function to format full timestamp for title attribute (hover tooltip)
function formatFullTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    timeZoneName: "short",
  }).format(new Date(timestamp));
}

interface ChatContainerProps {
  conversation: ChatMessage[];
  copiedMessageId: string | null;
  onCopyResponse: (response: ResponseExportInput, messageIndex: number) => void;
  onShareResponse: (response: ResponseExportInput, anchor: ShareAnchor) => void;
  onExamplePromptSelect: (prompt: string) => void;
  onApprovalUpdate?: (toolId: string, state: "approved" | "denied") => void;
  onContinuationUpdate?: (requestId: string, state: "stopped" | "continued") => void;
  /** Extra classes for the scroll container (e.g. a tighter pane in the bar). */
  className?: string;
  /** Extra classes for the message list; the bar uses this for denser padding. */
  contentClassName?: string;
}

export const ChatContainerV2 = React.memo(function ChatContainerV2({
  conversation,
  copiedMessageId,
  onCopyResponse,
  onShareResponse,
  onExamplePromptSelect,
  onApprovalUpdate,
  onContinuationUpdate,
  className,
  contentClassName,
}: ChatContainerProps) {
  // Memoize message list to prevent unnecessary re-renders
  const messageList = React.useMemo(
    () =>
      conversation.map((msg, index) => {
        const previousMsg = index > 0 ? conversation[index - 1] : null;
        const showTimestamp = shouldShowTimestamp(msg, previousMsg);
        // The question a reply answers: the nearest user message before it.
        const question =
          msg.role === "assistant"
            ? conversation
                .slice(0, index)
                .reverse()
                .find((m) => m.role === "user")?.content
            : undefined;

        return (
          <div key={`msg-container-${index}-${msg.timestamp || Date.now()}`}>
            {/* Timestamp header - minimal text-only */}
            {showTimestamp && msg.timestamp && (
              <div className="flex justify-center my-3">
                <span
                  className="text-[11px] text-muted-foreground/60 cursor-default"
                  title={formatFullTimestamp(msg.timestamp)}
                >
                  {formatMessageTimestamp(msg.timestamp)}
                </span>
              </div>
            )}

            <ChatMessageComponent
              msg={msg}
              index={index}
              question={question}
              copiedMessageId={copiedMessageId}
              onCopyResponse={onCopyResponse}
              onShareResponse={onShareResponse}
              onApprovalUpdate={onApprovalUpdate}
              onContinuationUpdate={onContinuationUpdate}
            />
          </div>
        );
      }),
    [
      conversation,
      copiedMessageId,
      onCopyResponse,
      onShareResponse,
      onApprovalUpdate,
      onContinuationUpdate,
    ]
  );

  return (
    <Conversation className={cn("flex-1 min-h-0", className)}>
      {conversation.length === 0 ? (
        <ConversationEmptyState>
          <div className="flex flex-col items-center justify-center space-y-6 py-12">
            <div className="space-y-2 text-center">
              <h1 className="text-2xl font-semibold tracking-tight text-foreground">
                What can I help you with?
              </h1>
              <p className="text-sm text-muted-foreground">
                Desktop automation, web browsing, file management, and more.
              </p>
            </div>

            <ExamplePrompts onPromptSelect={onExamplePromptSelect} />
          </div>
        </ConversationEmptyState>
      ) : (
        <ConversationContent className={cn("gap-6 px-6 py-4", contentClassName)}>
          {messageList}
        </ConversationContent>
      )}
      <ConversationScrollButton />
    </Conversation>
  );
});
