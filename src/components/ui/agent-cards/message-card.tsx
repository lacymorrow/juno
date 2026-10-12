/**
 * MessageCard: a text or an email, as it will go out.
 *
 * The send gate's one card. It shows who it goes to (as Contacts resolved
 * them), the subject when there is one, the body, and one action: Send. Three
 * states, all decided by the backend: draft (waiting for "send it" or the
 * button), sending (approved, going out), sent (read back from the app).
 *
 * The same card is emitted by the tools as a finished tag (`<MessageCard
 * state="sent" ... />`) and drawn by the approval prompt with `onSend`. It
 * holds no state of its own beyond "the button was pressed" and reads nothing.
 *
 * A fourth, quiet state: access is off (`needsAccess`). One sentence and one
 * button that opens the right row of System Settings.
 *
 * Flat on purpose: system blue is the only accent, no gradients, no glow.
 */

import { invoke } from "@tauri-apps/api/core";
import { Check, Loader2 } from "lucide-react";
import { useCallback, useState } from "react";
import { COMMANDS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

export type MessageCardState = "draft" | "sending" | "sent";

export interface MessageCardProps {
  state?: MessageCardState | string;
  /** "text" or "email". */
  kind?: "text" | "email" | string;
  /** The name Contacts resolved. */
  to?: string;
  /** The number or address it goes to. */
  address?: string;
  subject?: string;
  body?: string;
  /** The draft's one action. Only drawn while the draft waits. */
  onSend?: () => void;
  /** Set when access is off. Names what is needed: "Full Disk Access", "Messages". */
  needsAccess?: string;
  /** The sentence that goes with it, written by the backend. */
  reason?: string;
  /** Where the one switch lives. Only System Settings links are followed. */
  settingsUrl?: string;
  className?: string;
}

const SETTINGS_PREFIX = "x-apple.systempreferences:";

function StateLine({ state }: { state: MessageCardState }) {
  if (state === "sent") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-[#0a84ff]">
        <Check className="h-3 w-3" strokeWidth={3} aria-hidden="true" />
        Sent
      </span>
    );
  }
  if (state === "sending") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
        <Loader2 className="h-3 w-3 animate-spin motion-reduce:animate-none" aria-hidden="true" />
        Sending
      </span>
    );
  }
  return <span className="text-xs text-muted-foreground">Say &ldquo;send it&rdquo;</span>;
}

function normalizeState(state: string | undefined): MessageCardState {
  return state === "sending" || state === "sent" ? state : "draft";
}

export function MessageCard({
  state,
  kind = "text",
  to,
  address,
  subject,
  body,
  onSend,
  needsAccess,
  reason,
  settingsUrl,
  className,
}: MessageCardProps) {
  const [pressed, setPressed] = useState(false);
  const [opening, setOpening] = useState(false);
  const current = normalizeState(state);

  const openSettings = useCallback(async () => {
    if (!settingsUrl || !settingsUrl.startsWith(SETTINGS_PREFIX)) return;
    setOpening(true);
    try {
      await invoke(COMMANDS.DESKTOP_OPEN_URL, { url: settingsUrl });
    } catch (err) {
      console.error("MessageCard: could not open System Settings:", err);
    } finally {
      setOpening(false);
    }
  }, [settingsUrl]);

  if (needsAccess) {
    const canOpen = !!settingsUrl && settingsUrl.startsWith(SETTINGS_PREFIX);
    return (
      <div
        className={cn("flex items-center justify-between gap-3 rounded-xl border bg-card p-4", className)}
        data-testid="message-card"
        data-state="needs-access"
      >
        <p className="text-sm">{reason || `Juno needs your OK to use ${needsAccess}.`}</p>
        {canOpen ? (
          <button
            type="button"
            onClick={() => void openSettings()}
            disabled={opening}
            className={cn(
              "h-7 shrink-0 rounded-md bg-[#0a84ff] px-3 text-xs font-medium text-white",
              "hover:bg-[#0a84ff]/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              "disabled:opacity-50",
            )}
          >
            Turn on {needsAccess}
          </button>
        ) : null}
      </div>
    );
  }

  const send = () => {
    if (!onSend || pressed) return;
    setPressed(true);
    onSend();
  };
  const showSend = current === "draft" && !!onSend;
  const label = kind === "email" ? "Email" : "Message";

  return (
    <div
      className={cn("rounded-xl border bg-card p-4", className)}
      data-testid="message-card"
      data-state={current}
      aria-label={`${label} to ${to ?? address ?? "someone"}`}
      role="group"
    >
      <div className="flex items-baseline justify-between gap-3">
        <div className="min-w-0">
          <span className="text-xs text-muted-foreground">To </span>
          <span className="text-sm font-medium">{to || address}</span>
          {address && to && address !== to ? (
            <span className="ml-1.5 truncate text-xs text-muted-foreground">{address}</span>
          ) : null}
        </div>
        <StateLine state={current} />
      </div>

      {subject ? <div className="mt-2 text-sm font-medium">{subject}</div> : null}

      <p className={cn("mt-2 whitespace-pre-wrap break-words text-sm", current === "sent" && "text-muted-foreground")}>
        {body}
      </p>

      {showSend ? (
        <div className="mt-3 flex justify-end">
          <button
            type="button"
            onClick={send}
            disabled={pressed}
            className={cn(
              "h-7 rounded-md bg-[#0a84ff] px-4 text-xs font-medium text-white",
              "hover:bg-[#0a84ff]/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              "disabled:opacity-50",
            )}
          >
            Send
          </button>
        </div>
      ) : null}
    </div>
  );
}
