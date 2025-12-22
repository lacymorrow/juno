import React from "react";
import { DogIcon } from "lucide-react";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ui/conversation";
import {
  ChatMessageComponent,
  type ChatMessage,
} from "@/components/ChatMessage";
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
  userHasScrolledUp: boolean;
  lastScrollTime: number;
  setUserHasScrolledUp: (scrolled: boolean) => void;
  setLastScrollTime: (time: number) => void;
  onCopyResponse: (content: string, messageIndex: number) => void;
  onSaveResponse: (
    content: string,
    format: "html" | "markdown",
    messageIndex: number
  ) => void;
  onExamplePromptSelect: (prompt: string) => void;
}

export const ChatContainer = React.memo(function ChatContainer({
  conversation,
  copyingMessageId,
  savingMessageId,
  // Props below are maintained for interface compatibility but handled internally by Conversation component
  onCopyResponse,
  onSaveResponse,
  onExamplePromptSelect,
}: ChatContainerProps) {
  // Memoize message list to prevent unnecessary re-renders
  const messageList = React.useMemo(
    () =>
      conversation.map((msg, index) => {
        const previousMsg = index > 0 ? conversation[index - 1] : null;
        const showTimestamp = shouldShowTimestamp(msg, previousMsg);

        return (
          <div key={`msg-container-${index}-${msg.timestamp || Date.now()}`}>
            {/* Timestamp header - show when needed, similar to Slack/Apple Messages */}
            {showTimestamp && msg.timestamp && (
              <div className="flex justify-center my-4">
                <span
                  className="text-xs text-muted-foreground bg-background px-3 py-1 border rounded-full shadow-sm cursor-default"
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
    ]
  );

  return (
    <Conversation className="flex-1 min-h-0 mb-2">
      <ConversationContent>
        {conversation.length === 0 ? (
          /* Compact welcome message when conversation is empty */
          <ConversationEmptyState>
            <div className="space-y-1">
              <DogIcon size={16} className="text-blue-500 mx-auto" />
              <div>
                <h2 className="text-sm font-semibold">Juno AI</h2>
                <p className="text-xs text-muted-foreground">
                  AI desktop assistant
                </p>
              </div>
            </div>

            {/* Compact Example Prompts */}
            <ExamplePrompts onPromptSelect={onExamplePromptSelect} />
          </ConversationEmptyState>
        ) : (
          messageList
        )}
      </ConversationContent>
      <ConversationScrollButton />
    </Conversation>
  );
});
