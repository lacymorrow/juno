import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

export interface ReplyChip {
  id: string;
  label: string;
}

/**
 * Suggested replies under a locally served answer. Display only: the backend
 * decides what the chips are, when they expire, and what a tap does.
 */
export function ReplyChips() {
  const [chips, setChips] = useState<ReplyChip[]>([]);

  useEventListener<{ chips: ReplyChip[] }>(EVENTS.CHIPS_REPLY_CHIPS, (payload) => {
    setChips(payload?.chips ?? []);
  });

  if (chips.length === 0) return null;

  const tap = (id: string) => {
    // The backend clears or replaces the row; it is not hidden early here.
    invoke(COMMANDS.WINDOWS_RUN_REPLY_CHIP, { id }).catch((err) =>
      console.error("Failed to run the suggestion:", err),
    );
  };

  return (
    <div
      role="group"
      aria-label="Suggested replies"
      data-testid="reply-chips"
      className="flex flex-wrap gap-2 px-6 pb-3"
    >
      {chips.map((chip) => (
        <button
          key={chip.id}
          type="button"
          onClick={() => tap(chip.id)}
          className="rounded-full border border-[#0A84FF]/40 px-3 py-1 text-[13px] text-[#0A84FF] transition-colors hover:bg-[#0A84FF]/10 focus-visible:outline-2 focus-visible:outline-[#0A84FF]"
        >
          {chip.label}
        </button>
      ))}
    </div>
  );
}
