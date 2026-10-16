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
import { AccessRow } from "./access-row";

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
    <AccessRow
      className={className}
      data-testid="permission-ask-card"
      data-state={pressed ? "continuing" : "waiting"}
      role="group"
      aria-label="Permission needed"
      line={line || "I need your OK for that."}
      action="Continue"
      onAction={onContinue ? press : undefined}
      disabled={pressed}
    />
  );
}
