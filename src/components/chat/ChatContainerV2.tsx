import React from "react";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ai-elements/conversation";
import { ChatMessageComponent } from "@/components/ChatMessageV2";
import type { ChatMessage } from "@/types/chat";
import { ExamplePrompts } from "@/components/ExamplePrompts";

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
  copyingMessageId: string | null;
  savingMessageId: string | null;
  onCopyResponse: (content: string, messageIndex: number) => void;
  onSaveResponse: (
    content: string,
    format: "html" | "markdown",
    messageIndex: number
  ) => void;
  onExamplePromptSelect: (prompt: string) => void;
  onApprovalUpdate?: (toolId: string, state: "approved" | "denied") => void;
  onContinuationUpdate?: (requestId: string, state: "stopped" | "continued") => void;
}

export const ChatContainerV2 = React.memo(function ChatContainerV2({
  conversation,
  copyingMessageId,
  savingMessageId,
  onCopyResponse,
  onSaveResponse,
  onExamplePromptSelect,
  onApprovalUpdate,
  onContinuationUpdate,
}: ChatContainerProps) {
  // Memoize message list to prevent unnecessary re-renders
  const messageList = React.useMemo(
    () =>
      conversation.map((msg, index) => {
        const previousMsg = index > 0 ? conversation[index - 1] : null;
        const showTimestamp = shouldShowTimestamp(msg, previousMsg);

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
              copyingMessageId={copyingMessageId}
              savingMessageId={savingMessageId}
              onCopyResponse={onCopyResponse}
              onSaveResponse={onSaveResponse}
              onApprovalUpdate={onApprovalUpdate}
              onContinuationUpdate={onContinuationUpdate}
            />
          </div>
        );
      }),
    [
      conversation,
      copyingMessageId,
      savingMessageId,
      onCopyResponse,
      onSaveResponse,
      onApprovalUpdate,
      onContinuationUpdate,
    ]
  );

  return (
    <Conversation className="flex-1 min-h-0">
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
        <ConversationContent className="gap-6 px-6 py-4">
          {messageList}
        </ConversationContent>
      )}
      <ConversationScrollButton />
    </Conversation>
  );
});
