import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Tell Rust whether the bar is holding the conversation itself (the Pill's
 * chat pane, the Island's chat face).
 *
 * Screen-share pickers (the macOS one, Zoom, Chrome's window list) only offer
 * windows at the normal level, so a floating bar showing a chat was shaded on
 * hover and never offered. `bar_stacking` drops the bar to the normal level
 * while this is true; the decision stays there, this only reports the fact.
 * Unmounting reports closed, so a look being swapped out never leaves the bar
 * stuck at the normal level.
 */
export function useBarChatOpen(open: boolean): void {
  useEffect(() => {
    void invoke(COMMANDS.BAR_SET_BAR_CHAT_OPEN, { open }).catch(() => {});
    if (!open) return;
    return () => {
      void invoke(COMMANDS.BAR_SET_BAR_CHAT_OPEN, { open: false }).catch(() => {});
    };
  }, [open]);
}
