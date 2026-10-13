import { useEffect, useRef } from "react";

/**
 * Something inside the window that answers Escape itself is open: a dialog,
 * a menu, a listbox, or a popover. Its own dismissal comes first.
 */
const OWNS_ESCAPE =
  '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], [data-radix-popper-content-wrapper]';

/**
 * # Escape steps this window down
 *
 * Escape walks Juno down one rung per press. In a window that is a rung of
 * its own (the big chat, settings), a press this window is focused for is
 * that window's to answer: `onEscape` puts it away.
 *
 * A press something inside the window already handled is left alone, which
 * is how the shortcut recorder, the skill autocomplete and every dialog keep
 * their own Escape. It listens on `window` in the bubble phase, so React's
 * handlers (which run at the root, before it) have already had their say.
 */
export function useEscapeStepsDown(onEscape: () => void): void {
  const latest = useRef(onEscape);
  latest.current = onEscape;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented || e.repeat) return;
      if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
      if (document.querySelector(OWNS_ESCAPE)) return;
      latest.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
