/**
 * PermissionAskCard: "I need Reminders for that." and one button, Continue.
 *
 * Drawn by the permission preflight before a Mac app tool needs something
 * macOS has never been asked about. The system dialog comes up only after the
 * person presses Continue or says "continue", so nothing lands unannounced.
 * The line is written by the backend; this card holds no state beyond "the
 * button was pressed" and reads nothing.
 *
 * Same shape as MessageCard's draft state. Flat on purpose: system blue is
 * the only accent, no gradients, no glow.
 */

import { useState } from "react";
import { cn } from "@/lib/utils";

export interface PermissionAskCardProps {
  /** The one line, written by the backend: "I need Reminders for that." */
  line?: string;
  /** The one action. Only drawn when there is something to call. */
  onContinue?: () => void;
  className?: string;
}

export function PermissionAskCard({ line, onContinue, className }: PermissionAskCardProps) {
  const [pressed, setPressed] = useState(false);

  const press = () => {
    if (!onContinue || pressed) return;
    setPressed(true);
    onContinue();
  };

  return (
    <div
      className={cn("flex items-center justify-between gap-3 rounded-xl border bg-card p-4", className)}
      data-testid="permission-ask-card"
      data-state={pressed ? "continuing" : "waiting"}
      role="group"
      aria-label="Permission needed"
    >
      <p className="text-sm">{line || "I need your OK for that."}</p>
      {onContinue ? (
        <button
          type="button"
          onClick={press}
          disabled={pressed}
          className={cn(
            "h-7 shrink-0 rounded-md bg-[#0a84ff] px-4 text-xs font-medium text-white",
            "hover:bg-[#0a84ff]/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            "disabled:opacity-50",
          )}
        >
          Continue
        </button>
      ) : null}
    </div>
  );
}
