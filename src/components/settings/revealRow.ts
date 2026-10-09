import { SETTINGS_ROW_ID_PREFIX } from "./ui";

/**
 * Point at one settings row: scroll it into view and tint it system blue for
 * a moment. Used by sidebar search and by the agent's `settings` tool
 * ("show me where the model setting is").
 *
 * Flat on purpose, the way System Settings marks a search hit: a tint that
 * fades in and out (the row's own `transition-colors`), no ring, no glow, no
 * spring. Reduced motion jumps instead of scrolling and drops the fade.
 */

/** The tint. Plain class strings so Tailwind generates them. */
export const ROW_FLASH_CLASSES = ["bg-[#007AFF]/15", "dark:bg-[#0A84FF]/25"];

/** How long the tint stays before it fades. */
export const ROW_FLASH_MS = 1600;

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
