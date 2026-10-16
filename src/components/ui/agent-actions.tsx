/**
 * Interactive components for agent JSX responses.
 *
 * - QueryButton: Submit a new query to the agent
 * - OpenButton: Open a URL, file path or app
 * - CopyButton: Copy text to clipboard
 * - Actions: the row they sit in, at most one primary
 *
 * All three are the shadcn `Button` at `size="card"` (24px, no icon), so they
 * take their colours from the Mac tokens. `primary` is the blue one.
 * Components are registered in availableComponents for react-jsx-parser.
 */

import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { COMMANDS } from "@/lib/constants.generated";
import { Children, cloneElement, isValidElement, useState, useCallback, useEffect, useRef } from "react";

/**
 * Dispatch a query on behalf of an agent-rendered component.
 *
 * Uses the same unified submission pipeline as the chat input and the bar,
 * so the chat window receives the user message, the floating bar shows its
 * submitting state, and the promise resolves once the query is accepted
 * rather than after the entire agent run.
 */
async function dispatchQuery(query: string): Promise<void> {
  await invoke(COMMANDS.AGENT_DISPATCH_QUERY, { query });
}

/**
 * Keep a single pending reset timer per button and clear it on unmount so a
 * late `setStatus` never fires against an unmounted component.
 */
function useStatusResetTimer(reset: () => void) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return useCallback(
    (delayMs: number) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(reset, delayMs);
    },
    [reset],
  );
}

// ============================================================
// Actions: the row the buttons sit in
// ============================================================

/**
 * A row of buttons. Left-aligned under prose, right-aligned in a card footer.
 * Only the first `primary` stays primary: a card has one main action.
 */
export function Actions({ children, className }: { children?: React.ReactNode; className?: string }) {
  let seenPrimary = false;
  const items = Children.map(children, (child) => {
    if (!isValidElement<{ primary?: boolean }>(child) || !child.props.primary) return child;
    if (!seenPrimary) {
      seenPrimary = true;
      return child;
    }
    return cloneElement(child, { primary: false });
  });

  return (
    <div
      data-slot="actions"
      className={cn("flex flex-wrap items-center gap-2 [[data-slot=card-footer]_&]:justify-end", className)}
    >
      {items}
    </div>
  );
}

// ============================================================
// QueryButton — submit a new query to the agent
// ============================================================

interface ButtonCommonProps {
  /** The one main action of the card or row. */
  primary?: boolean;
  /** Older answers pass a variant or size; the card button has one of each. */
  variant?: string;
  size?: string;
  className?: string;
}

interface QueryButtonProps extends ButtonCommonProps {
  /** The query to submit */
  query: string;
  /** Button label */
  label?: string;
}

export function QueryButton({ query, label, primary, className }: QueryButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "sent" | "error">("idle");
  const resetToIdle = useCallback(() => setStatus("idle"), []);
  const scheduleReset = useStatusResetTimer(resetToIdle);

  const handleClick = useCallback(async () => {
    setStatus("loading");
    try {
      await dispatchQuery(query);
      // The new user message appearing is the feedback; the button just comes back.
      setStatus("sent");
      scheduleReset(1500);
    } catch (err) {
      console.error("QueryButton: Failed to submit query:", err);
      setStatus("error");
      scheduleReset(3000);
    }
  }, [query, scheduleReset]);

  return (
    <span className="inline-flex flex-col items-end gap-1">
      <Button
        type="button"
        size="card"
        variant={primary ? "default" : "secondary"}
        onClick={handleClick}
        disabled={status === "loading"}
        className={className}
      >
        {label || query}
      </Button>
      {status === "error" && (
        <span className="text-[11px] text-muted-foreground">Could not send that. Try again.</span>
      )}
    </span>
  );
}

// ============================================================
// OpenButton — open a URL or file path
// ============================================================

interface OpenButtonProps extends ButtonCommonProps {
  /** URL to open in default browser */
  url?: string;
  /** File path to open — converted to file:// URL */
  path?: string;
  /** Application name to open (e.g. "Safari", "Finder") */
  app?: string;
  /** Button label */
  label?: string;
}

export function OpenButton({ url, path, app, label, primary, className }: OpenButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");

  const handleClick = useCallback(async () => {
    setStatus("loading");
    try {
      if (app) {
        await invoke(COMMANDS.DESKTOP_OPEN_APPLICATION, { appName: app });
      } else if (path) {
        // The model writes paths the way people say them, "~/Downloads".
        // Dropping the tilde used to send file:///Downloads, a folder that
        // does not exist, so the button looked pressed and nothing opened.
        const expanded = path.startsWith("~")
          ? path.replace(/^~/, (await homeDir()).replace(/\/$/, ""))
          : path;
        const fileUrl = expanded.startsWith("file://") ? expanded : `file://${expanded}`;
        await invoke(COMMANDS.DESKTOP_OPEN_URL, { url: fileUrl });
      } else if (url) {
        await invoke(COMMANDS.DESKTOP_OPEN_URL, { url });
      }
      setStatus("idle");
    } catch (err) {
      console.error("OpenButton: Failed to open:", err);
      setStatus("error");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [url, path, app]);

  const target = app || path || url || "";
  const displayLabel = label || (app ? app : path ? path.split("/").pop() : target);

  return (
    <span className="inline-flex flex-col items-end gap-1">
      <Button
        type="button"
        size="card"
        variant={primary ? "default" : "secondary"}
        onClick={handleClick}
        disabled={status === "loading"}
        className={className}
      >
        {displayLabel}
      </Button>
      {status === "error" && (
        <span className="text-[11px] text-muted-foreground">Could not open that.</span>
      )}
    </span>
  );
}

// ============================================================
// CopyButton — copy text to clipboard
// ============================================================

interface CopyButtonProps extends ButtonCommonProps {
  /** Text to copy */
  text: string;
  /** Button label */
  label?: string;
}

export function CopyButton({ text, label = "Copy", primary, className }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const handleClick = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("CopyButton: Failed to copy:", err);
    }
  }, [text]);

  // Both words are laid out in one cell, so the button keeps the wider width.
  return (
    <Button
      type="button"
      size="card"
      variant={primary ? "default" : "secondary"}
      onClick={handleClick}
      className={className}
    >
      <span className="grid">
        <span className={cn("col-start-1 row-start-1", copied && "invisible")}>{label}</span>
        <span className={cn("col-start-1 row-start-1", !copied && "invisible")} aria-hidden={!copied}>
          Copied
        </span>
      </span>
    </Button>
  );
}
