import React from "react";
import { DogIcon } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  ChatMessageComponent,
  type ChatMessage,
} from "@/components/ChatMessage";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import { useChatScrolling } from "@/hooks/useChatScrolling";

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
  userHasScrolledUp,
  lastScrollTime,
  setUserHasScrolledUp,
  setLastScrollTime,
  onCopyResponse,
  onSaveResponse,
  onExamplePromptSelect,
}: ChatContainerProps) {
  const { conversationEndRef, scrollAreaRef } = useChatScrolling({
    conversation,
    userHasScrolledUp,
    lastScrollTime,
    setUserHasScrolledUp,
    setLastScrollTime,
  });

  // Memoize message list to prevent unnecessary re-renders
  const messageList = React.useMemo(
    () =>
      conversation.map((msg, index) => {
        const previousMsg = index > 0 ? conversation[index - 1] : null;
        const showTimestamp = shouldShowTimestamp(msg, previousMsg);

        return (
          <div key={`msg-container-${index}-${msg.timestamp || Date.now()}`}>
            {/* Enhanced Timestamp header - macOS Messages style */}
            {showTimestamp && msg.timestamp && (
              <div className="flex justify-center my-6">
                <span
                  className="text-xs text-muted-foreground bg-background/80 backdrop-blur-sm px-4 py-2 border border-border/30 rounded-full shadow-sm cursor-default transition-all duration-200 hover:bg-background/90 hover:shadow-md font-medium"
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
    <ScrollArea className="flex-1 min-h-0 mb-2 -mr-4 pr-4" ref={scrollAreaRef}>
      {conversation.length === 0 ? (
        /* Enhanced welcome message with macOS styling */
        <div className="flex flex-col items-center justify-center h-full text-center space-y-6 p-6">
          <div className="space-y-4">
            {/* Enhanced App Identity */}
            <div className="flex items-center justify-center gap-3 px-6 py-3 rounded-2xl bg-gradient-to-r from-blue-50 to-indigo-50 dark:from-blue-950/50 dark:to-indigo-950/50 border border-blue-200/50 dark:border-blue-800/50 backdrop-blur-sm shadow-sm">
              <DogIcon size={24} className="text-blue-600 dark:text-blue-400" />
              <div className="text-left">
                <h2 className="text-lg font-semibold bg-gradient-to-r from-blue-700 to-indigo-700 dark:from-blue-300 dark:to-indigo-300 bg-clip-text text-transparent">
                  Juno AI
                </h2>
                <p className="text-xs text-blue-600/70 dark:text-blue-400/70 font-medium">
                  AI desktop assistant
                </p>
              </div>
            </div>

            {/* Welcome Text */}
            <div className="space-y-2">
              <h3 className="text-base font-medium text-foreground">
                Welcome to your AI assistant
              </h3>
              <p className="text-sm text-muted-foreground max-w-md mx-auto leading-relaxed">
                Ask me anything, control your Mac, or try one of the examples below to get started.
              </p>
            </div>
          </div>

          {/* Enhanced Example Prompts */}
          <div className="w-full max-w-2xl">
            <ExamplePrompts onPromptSelect={onExamplePromptSelect} />
          </div>
        </div>
      ) : (
        <div className="space-y-4 py-4">
          {messageList}
        </div>
      )}
      <div ref={conversationEndRef} />
    </ScrollArea>
  );
});
