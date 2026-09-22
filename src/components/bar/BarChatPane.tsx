import { Plus, Settings, SquareArrowOutUpRight, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { ChatContainerV2 } from "@/components/chat/ChatContainerV2";
import type { BackendStatus } from "@/components/ExamplePrompts";
import type { ChatMessage, ResponseExportInput } from "@/types/chat";
import type { ShareAnchor } from "@/hooks/useConversation";
import { cn } from "@/lib/utils";
import { BAR_DEPTH_GLOW } from "@/components/bar/barAppearance";

interface BarChatPaneProps {
  messages: ChatMessage[];
  isProcessing: boolean;
  /** Gates the empty-state example prompts, same as the main window. */
  backendStatus: BackendStatus;
  height: number;
  copiedMessageId: string | null;
  onCopyResponse: (response: ResponseExportInput, index: number) => void;
  onShareResponse: (response: ResponseExportInput, anchor: ShareAnchor) => void;
  /** An example prompt clicked in the empty state; sends it like a typed follow-up. */
  onExamplePromptSelect: (prompt: string) => void;
  onApprovalUpdate: (toolId: string, state: "approved" | "denied") => void;
  onContinuationUpdate: (requestId: string, state: "stopped" | "continued") => void;
  onDismiss: () => void;
  onNewChat: () => void;
}

const headerButton =
  "flex size-6 items-center justify-center rounded-full text-white/35 transition-colors hover:bg-white/[0.08] hover:text-white/80";

/** The full-size chat window; it shows this same conversation. */
const openInWindow = () => {
  invoke("open_main_window").catch((err) =>
    console.error("Failed to open the main window:", err),
  );
};

const openSettings = () => {
  invoke("open_settings_window").catch((err) =>
    console.error("Failed to open settings:", err),
  );
};

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
  backendStatus,
  height,
  copiedMessageId,
  onCopyResponse,
  onShareResponse,
  onExamplePromptSelect,
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
        "border border-white/10 bg-neutral-950/90 text-foreground backdrop-blur-xl",
      )}
      // Same depth glow as the pill so the pane never vanishes on a dark
      // background either.
      style={{ height, boxShadow: BAR_DEPTH_GLOW, animation: "fbar-content-in 0.25s ease-out both" }}
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
            className={headerButton}
          >
            <Plus className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={openInWindow}
            aria-label="Open in window"
            title="Open in window"
            className={headerButton}
          >
            <SquareArrowOutUpRight className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={openSettings}
            aria-label="Settings"
            title="Settings"
            className={headerButton}
          >
            <Settings className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={onDismiss}
            aria-label="Dismiss conversation"
            title="Dismiss (Esc)"
            className={headerButton}
          >
            <X className="size-3.5" />
          </button>
        </div>
      </header>

      {/* select-text undoes the bar root's select-none so replies read like any Mac text */}
      <div data-no-drag className="flex min-h-0 flex-1 cursor-auto select-text flex-col">
        <ChatContainerV2
          conversation={messages}
          copiedMessageId={copiedMessageId}
          onCopyResponse={onCopyResponse}
          onShareResponse={onShareResponse}
          onExamplePromptSelect={onExamplePromptSelect}
          backendStatus={backendStatus}
          onApprovalUpdate={onApprovalUpdate}
          onContinuationUpdate={onContinuationUpdate}
          contentClassName="gap-4 px-4 py-3 text-[13px]"
        />
      </div>
    </section>
  );
}
