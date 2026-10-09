import React from "react";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
  ConversationScrollOnSend,
} from "@/components/ai-elements/conversation";
import {
  ChatMessageComponent,
  awaitsApproval,
  isToolRow,
} from "@/components/ChatMessageV2";
import { isDevelopment } from "@/lib";
import type { ChatMessage, ResponseExportInput } from "@/types/chat";
import {
  formatTurnSummary,
  summarizeTurn,
  turnScreenshots,
} from "@/lib/turn-summary";
import type { ShareAnchor } from "@/hooks/useConversation";
import { ExamplePrompts, type BackendStatus } from "@/components/ExamplePrompts";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { PermissionNotice } from "@/components/permissions/PermissionNotice";
import { cn } from "@/lib/utils";
import { ReplyChips } from "@/components/chat/ReplyChips";

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

/**
 * Whether a message is drawn at all.
 *
 * Tool calls are development furniture: the transcript a person reads is their
 * question, what Juno said back, and anything Juno had to ask permission for.
 * The one tool row that survives production is an approval, because hiding it
 * would leave the agent waiting on an answer the person was never asked for.
 */
function isVisible(msg: ChatMessage, showToolDetails: boolean): boolean {
  if (showToolDetails) return true;
  return !isToolRow(msg) || awaitsApproval(msg);
}

/**
 * True when the last thing that happened is work the transcript does not show.
 *
 * A turn that is nothing but tool calls would otherwise be an empty pane: the
 * assistant bubble, and the shimmer that comes with it, only opens on the
 * first chunk of text, and a model that goes straight to tools emits none. One
 * plain line stands in until the reply lands.
 */
function hasHiddenWorkInFlight(
  conversation: ChatMessage[],
  showToolDetails: boolean
): boolean {
  if (showToolDetails) return false;
  const last = conversation[conversation.length - 1];
  return !!last && isToolRow(last) && !awaitsApproval(last);
}

interface ChatContainerProps {
  conversation: ChatMessage[];
  copiedMessageId: string | null;
  onCopyResponse: (response: ResponseExportInput, messageIndex: number) => void;
  onShareResponse: (response: ResponseExportInput, anchor: ShareAnchor) => void;
  onExamplePromptSelect: (prompt: string) => void;
  /** Gates the example prompts: a loader while connecting, live once connected. */
  backendStatus?: BackendStatus;
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
  backendStatus = "connected",
  onApprovalUpdate,
  onContinuationUpdate,
  className,
  contentClassName,
}: ChatContainerProps) {
  // Tool cards are a development view. The backend owns the answer; this is one
  // read at mount, long before any tool call can arrive, and the default until
  // it resolves is the production one.
  const [showToolDetails, setShowToolDetails] = React.useState(false);

  React.useEffect(() => {
    isDevelopment()
      .then(setShowToolDetails)
      .catch(() => setShowToolDetails(false));
  }, []);

  // How many messages the person has sent. Sending is the one moment the
  // conversation should jump to the bottom whatever the reader was doing.
  const sentCount = React.useMemo(
    () => conversation.filter((m) => m.role === "user").length,
    [conversation],
  );

  // Empty means nobody has said anything yet. Status notes the app writes to
  // itself ("Connected…") do not count: they used to replace the example
  // prompts the moment the backend came up, so in the main window the buttons
  // only ever existed while nothing could be sent.
  const hasExchange = React.useMemo(
    () => conversation.some((m) => m.role === "user" || m.role === "assistant"),
    [conversation],
  );

  // Memoize message list to prevent unnecessary re-renders
  const messageList = React.useMemo(
    () => {
      // Filtering here rather than returning null from the renderer: each row
      // is a child of a `gap-6` column, so a row that renders nothing still
      // leaves its gap, and the timestamp divider above it would separate a
      // hole from a hole. Original indices ride along, because the copy and
      // share handlers and the turn summary are both indexed into the whole
      // conversation, not into what is on screen.
      const visible = conversation
        .map((msg, index) => ({ msg, index }))
        .filter(({ msg }) => isVisible(msg, showToolDetails));

      return visible.map(({ msg, index }, position) => {
        const previousMsg = position > 0 ? visible[position - 1].msg : null;
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
          // The key has to hold still across a streaming update. A `Date.now()`
          // fallback changes on every render, which remounts the row mid-stream
          // and throws away its scroll height, selection and open tool cards.
          <div key={msg.messageId ?? `msg-${index}-${msg.timestamp ?? "no-ts"}`}>
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
              showToolDetails={showToolDetails}
              // The captures this turn made. Gathered here because only the
              // container can see the turn: the images arrive on tool rows,
              // which the transcript does not draw, and a reply that cannot
              // reach them is why a screenshot was never visible outside
              // development.
              screenshots={
                msg.role === "assistant"
                  ? turnScreenshots(conversation, index)
                  : undefined
              }
            />

            {/* What the turn cost, for turns that spent anything. A reply that
                used no tools returns null and renders nothing, so ordinary
                chat stays clean and only real work reports itself. */}
            {(() => {
              const summary = summarizeTurn(conversation, index);
              if (!summary) return null;
              return (
                <div className="mt-1 text-[11px] text-muted-foreground/60 cursor-default tabular-nums">
                  {formatTurnSummary(summary)}
                </div>
              );
            })()}
          </div>
        );
      });
    },
    [
      conversation,
      copiedMessageId,
      onCopyResponse,
      onShareResponse,
      onApprovalUpdate,
      onContinuationUpdate,
      showToolDetails,
    ]
  );

  return (
    // The conversation scrolls; the background-mode notices sit under it, so a
    // question about taking the mouse never scrolls out of sight.
    <div className="flex min-h-0 flex-1 flex-col">
      <Conversation className={cn("flex-1 min-h-0", className)}>
        {/* Counting sent messages rather than watching the array: the count
            changes exactly once per send, while the array changes on every
            streamed token. */}
        <ConversationScrollOnSend signal={sentCount} />
        {!hasExchange ? (
          // Scrolls, and centres with auto margins rather than flex centring:
          // flex centring clips both ends once the content is taller than the
          // box, which in the bar's 360px pane it is as soon as the dev
          // commands drawer opens. Auto margins centre when there is room and
          // fall back to a normal scroll when there is not.
          <ConversationEmptyState className="justify-start overflow-y-auto">
            <div className="m-auto flex flex-col items-center justify-center space-y-6 py-6">
              <div className="space-y-2 text-center">
                <h1 className="text-2xl font-semibold tracking-tight text-foreground">
                  Let's get something done.
                </h1>
                <p className="text-sm text-muted-foreground">
                  Pick one, or just tell me what you need.
                </p>
              </div>

              <ExamplePrompts
                onPromptSelect={onExamplePromptSelect}
                backendStatus={backendStatus}
              />
            </div>
          </ConversationEmptyState>
        ) : (
          <ConversationContent className={cn("gap-6 px-6 py-4", contentClassName)}>
            {messageList}
            {hasHiddenWorkInFlight(conversation, showToolDetails) && (
              <div className="text-[11px] text-muted-foreground/60 cursor-default">
                Working...
              </div>
            )}
          </ConversationContent>
        )}
        <ConversationScrollButton />
      </Conversation>
      <ReplyChips />
      <InputControlNotices />
      <PermissionNotice />
    </div>
  );
});
