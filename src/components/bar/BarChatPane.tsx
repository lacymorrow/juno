import { Plus, X } from "lucide-react";
import { ChatContainerV2 } from "@/components/chat/ChatContainerV2";
import type { ChatMessage } from "@/types/chat";
import { cn } from "@/lib/utils";

interface BarChatPaneProps {
  messages: ChatMessage[];
  isProcessing: boolean;
  height: number;
  copyingMessageId: string | null;
  savingMessageId: string | null;
  onCopyResponse: (content: string, index: number) => void;
  onSaveResponse: (content: string, format: "html" | "markdown", index: number) => void;
  onApprovalUpdate: (toolId: string, state: "approved" | "denied") => void;
  onContinuationUpdate: (requestId: string, state: "stopped" | "continued") => void;
  onDismiss: () => void;
  onNewChat: () => void;
}

const noopPromptSelect = () => {};

/**
 * The conversation, docked under the floating bar.
 *
 * This is the same chat pane the main window renders (`ChatContainerV2` →
 * `ChatMessageComponent`), wrapped in a scoped `.dark` theme so every shadcn
 * token — bubbles, tool cards, reasoning, approvals — picks up the bar's dark
 * palette without touching the main window's light theme.
 *
 * The header is a drag handle; the message list opts out of window dragging
 * (`data-no-drag`) so text can be selected and links clicked.
 */
export function BarChatPane({
  messages,
  isProcessing,
  height,
  copyingMessageId,
  savingMessageId,
  onCopyResponse,
  onSaveResponse,
  onApprovalUpdate,
  onContinuationUpdate,
  onDismiss,
  onNewChat,
}: BarChatPaneProps) {
  return (
    <section
      role="region"
      aria-label="Conversation"
      data-testid="bar-chat-pane"
      className={cn(
        "dark flex w-[419px] flex-col overflow-hidden rounded-2xl",
        "border border-white/10 bg-neutral-950/90 text-foreground shadow-2xl backdrop-blur-xl",
      )}
      style={{ height, animation: "fbar-content-in 0.25s ease-out both" }}
    >
      <header className="flex h-8 shrink-0 select-none items-center justify-between border-b border-white/[0.06] pl-4 pr-2">
        <span
          className="text-[11px] tracking-[0.04em] text-white/35"
          data-testid="bar-chat-pane-status"
        >
          {isProcessing ? "working" : "esc to close"}
        </span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={onNewChat}
            aria-label="New chat"
            title="New chat"
            className="flex size-6 items-center justify-center rounded-full text-white/35 transition-colors hover:bg-white/[0.08] hover:text-white/80"
          >
            <Plus className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={onDismiss}
            aria-label="Dismiss conversation"
            title="Dismiss (Esc)"
            className="flex size-6 items-center justify-center rounded-full text-white/35 transition-colors hover:bg-white/[0.08] hover:text-white/80"
          >
            <X className="size-3.5" />
          </button>
        </div>
      </header>

      <div data-no-drag className="flex min-h-0 flex-1 cursor-auto flex-col">
        <ChatContainerV2
          conversation={messages}
          copyingMessageId={copyingMessageId}
          savingMessageId={savingMessageId}
          onCopyResponse={onCopyResponse}
          onSaveResponse={onSaveResponse}
          onExamplePromptSelect={noopPromptSelect}
          onApprovalUpdate={onApprovalUpdate}
          onContinuationUpdate={onContinuationUpdate}
          contentClassName="gap-4 px-4 py-3 text-[13px]"
        />
      </div>
    </section>
  );
}
