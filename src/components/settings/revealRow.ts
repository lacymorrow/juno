import { SETTINGS_ROW_ID_PREFIX } from "./ui";

/**
 * Point at one settings row: scroll it into view and tint it system blue for
 * a moment. Used by sidebar search and by the agent's `settings` tool
 * ("show me where the model setting is").
 *
 * Flat on purpose: an outline and a tint, no glow, no spring. The tint alone
 * read as part of the design, so the outline makes it unmistakable. Reduced
 * motion jumps instead of scrolling and keeps the outline but drops the fade.
 */

/**
 * The mark: a 2px system blue outline drawn inside the row (the row's base
 * classes reserve it transparent) over a stronger tint. Plain class strings
 * so Tailwind generates them. Only colour changes, so the row's own
 * `transition-colors` is the fade; reduced motion removes it and the mark
 * simply disappears at the end.
 */
export const ROW_FLASH_CLASSES = [
  "bg-[#007AFF]/20",
  "dark:bg-[#0A84FF]/30",
  "outline-[#007AFF]",
  "dark:outline-[#0A84FF]",
];

/** How long the mark is held before it eases out. */
export const ROW_FLASH_MS = 2500;

/** How long to wait for a row whose section is still loading. */
const ROW_WAIT_MS = 2000;

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Tint a row, then take the tint away. */
export function flashRow(el: HTMLElement, ms = ROW_FLASH_MS): void {
  el.classList.add(...ROW_FLASH_CLASSES);
  setTimeout(() => el.classList.remove(...ROW_FLASH_CLASSES), ms);
}

/**
 * Find the row (waiting a frame at a time while its section renders), scroll
 * it to the middle of the pane and flash it. Returns a cancel for the wait;
 * a flash already shown runs its course.
 */
export function revealRow(rowId: string, waitMs = ROW_WAIT_MS): () => void {
  let cancelled = false;
  let frame = 0;
  const started = Date.now();

  const look = () => {
    if (cancelled) return;
    const el = document.getElementById(`${SETTINGS_ROW_ID_PREFIX}${rowId}`);
    if (el) {
      el.scrollIntoView?.({
        block: "center",
        behavior: prefersReducedMotion() ? "auto" : "smooth",
      });
      flashRow(el);
      return;
    }
    if (Date.now() - started < waitMs) frame = requestAnimationFrame(look);
  };

  frame = requestAnimationFrame(look);
  return () => {
    cancelled = true;
    cancelAnimationFrame(frame);
  };
}
