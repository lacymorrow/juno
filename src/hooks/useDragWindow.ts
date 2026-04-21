import { useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Interactive element selectors that should NOT trigger window dragging.
 * Clicking a button, typing in an input, etc. should work normally.
 * Includes `[role="button"]` for custom button components and
 * `[contenteditable]` for editable regions.
 */
const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable], [data-no-drag]';

/**
 * Hook that returns an `onMouseDown` handler for dragging the current window.
 *
 * Tauri's `data-tauri-drag-region` attribute is unreliable on macOS when the
 * window uses `transparent: true` + `decorations: false` — the OS doesn't
 * route mouse events through transparent webview areas. Calling
 * `window.startDragging()` programmatically on mousedown is the robust
 * alternative used by production Tauri apps.
 *
 * Usage:
 * ```tsx
 * const onDragMouseDown = useDragWindow();
 * return <div onMouseDown={onDragMouseDown}>...</div>;
 * ```
 */
export function useDragWindow() {
  return useCallback(async (e: React.MouseEvent) => {
    // Only trigger on primary (left) mouse button
    if (e.button !== 0) return;

    // Don't start a drag if the user clicked an interactive element
    const target = e.target as HTMLElement;
    if (target.closest(INTERACTIVE_SELECTOR)) return;

    // Prevent text selection while dragging
    e.preventDefault();

    try {
      await getCurrentWindow().startDragging();
    } catch (err) {
      // startDragging can fail if the window is already being moved or
      // if it's called outside a user gesture — safe to ignore.
      console.debug("startDragging failed:", err);
    }
  }, []);
}

/**
 * Hook for surfaces that are BOTH clickable AND draggable (e.g. orb, persona).
 *
 * Unlike `useDragWindow`, this variant only starts dragging after the mouse
 * moves beyond a pixel threshold. A quick tap (mousedown → mouseup with
 * little/no movement) passes through cleanly so `onClick` fires normally.
 *
 * Returns three handlers that must all be spread onto the element:
 * ```tsx
 * const dragHandlers = useDragWindowWithThreshold();
 * return <div onClick={handleClick} {...dragHandlers}>...</div>;
 * ```
 */
export function useDragWindowWithThreshold(threshold = 4) {
  const startPos = useRef<{ x: number; y: number } | null>(null);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement;
      if (target.closest(INTERACTIVE_SELECTOR)) return;
      startPos.current = { x: e.clientX, y: e.clientY };
    },
    [],
  );

  const onMouseMove = useCallback(
    async (e: React.MouseEvent) => {
      if (!startPos.current) return;
      const dx = e.clientX - startPos.current.x;
      const dy = e.clientY - startPos.current.y;
      if (Math.abs(dx) + Math.abs(dy) >= threshold) {
        startPos.current = null;
        e.preventDefault();
        try {
          await getCurrentWindow().startDragging();
        } catch {
          // safe to ignore
        }
      }
    },
    [threshold],
  );

  const onMouseUp = useCallback(() => {
    startPos.current = null;
  }, []);

  return { onMouseDown, onMouseMove, onMouseUp };
}
