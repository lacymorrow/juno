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
 * The mark: a 2px system blue outline drawn inside the row over a stronger
 * tint, set as inline styles. Classes lost this: the row reserves a
 * transparent outline with a class of its own, same specificity, so the
 * stylesheet order decided and transparent won. Inline always wins. Clearing
 * the styles hands the row back to its transparent outline, and the row's own
 * colour transition is the fade; reduced motion removes it and the mark
 * simply disappears at the end.
 */
export const ROW_FLASH_STYLE = {
  light: { color: "#007AFF", tint: "rgba(0, 122, 255, 0.2)" },
  dark: { color: "#0A84FF", tint: "rgba(10, 132, 255, 0.3)" },
} as const;

/**
 * The window's actual theme. The settings window scopes a `.dark` class on
 * its own root (`useSystemTheme`), so look up from the row; a `.light` scope
 * wins over the OS; with neither, the OS setting.
 */
function isDark(el: HTMLElement): boolean {
  const scope = el.closest(".dark, .light");
  if (scope) return scope.classList.contains("dark");
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

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

/** Mark a row, then take the mark away. */
export function flashRow(el: HTMLElement, ms = ROW_FLASH_MS): void {
  const look = isDark(el) ? ROW_FLASH_STYLE.dark : ROW_FLASH_STYLE.light;
  el.style.outline = `2px solid ${look.color}`;
  el.style.outlineOffset = "-2px";
  el.style.backgroundColor = look.tint;
  setTimeout(() => {
    el.style.outline = "";
    el.style.outlineOffset = "";
    el.style.backgroundColor = "";
  }, ms);
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
