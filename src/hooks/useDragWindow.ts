import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { COMMANDS } from "@/lib/constants.generated";

import { armBarSnap, settleBarSnap, useBarDisplayFollow } from "./useBarSnapWells";

/**
 * Interactive element selectors that should NOT trigger window dragging on a
 * surface where a press means "drag" straight away (the floating panels).
 * Without a movement threshold a drag through a button would eat its click,
 * so every control is excluded here.
 */
const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable], [data-no-drag]';

/**
 * What the BAR never drags from: text entry and explicit opt-outs.
 *
 * The bar's gesture has a movement threshold, so a press and release on a
 * button is still that button's click and only real movement becomes a drag.
 * That makes it safe to drag straight through a control, which is the point:
 * the bar is a small object you grab anywhere, not a window with a title bar.
 * Text entry is still excluded so a caret can be placed and text selected.
 */
const BAR_NO_DRAG_SELECTOR = "input, textarea, [contenteditable], [data-no-drag]";

/** Drag starts once the mouse has moved this far from where it went down. */
export const DRAG_THRESHOLD_PX = 4;

/**
 * Hook that returns an `onMouseDown` handler for dragging the current window.
 *
 * Tauri's `data-tauri-drag-region` attribute is unreliable on macOS when the
 * window uses `transparent: true` + `decorations: false` — the OS doesn't
 * route mouse events through transparent webview areas. Calling
 * `window.startDragging()` programmatically on mousedown is the robust
 * alternative used by production Tauri apps.
 *
 * For the bar window use `useBarDrag` instead: this variant drags the moment
 * the mouse goes down, which is right for a panel's chrome and wrong for a
 * small object whose whole surface is also a control.
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

export interface BarDragOptions {
  /**
   * The look is busy and must not be re-homed when the cursor changes display
   * (the Pill passes this while its chat pane is open or the agent is working).
   */
  displayFollowPaused?: boolean;
  /** Pixels of travel before a press becomes a drag. */
  threshold?: number;
}

export interface BarDrag {
  /**
   * Spread onto the look's root element. Capture phase, so a child that
   * handles its own mousedown cannot swallow the gesture.
   */
  dragProps: {
    onMouseDownCapture: (e: React.MouseEvent) => void;
    onMouseMove: (e: React.MouseEvent) => void;
    onMouseUp: () => void;
    onContextMenu: (e: React.MouseEvent) => void;
  };
  /**
   * Call first from the root's `onClickCapture`. Returns true when it has
   * swallowed the click that a finished drag would otherwise fire, in which
   * case the caller does nothing else.
   */
  swallowClickAfterDrag: (e: React.MouseEvent) => boolean;
}

/**
 * The one drag gesture for the bar window: drag from anywhere, land in a well.
 *
 * A mousedown anywhere except text entry arms a drag; moving past the
 * threshold hands the gesture to the OS window drag and swallows the click
 * that would otherwise fire on release. A press and release without movement
 * is an ordinary click on whatever was pressed.
 *
 * On release the window glides into the nearest gravity well. That part is
 * gated on the window's label inside `armBarSnap`/`settleBarSnap`, so this
 * hook stays usable anywhere while only the bar snaps.
 *
 * Every look calls this and nothing else, which is why a look added later
 * gets the wells with no wiring: the drag hook it would call anyway is the
 * drag hook that snaps.
 */
export function useBarDrag({
  displayFollowPaused = false,
  threshold = DRAG_THRESHOLD_PX,
}: BarDragOptions = {}): BarDrag {
  // Where the press landed inside the window, in logical px. It is both the
  // threshold's origin and the grab offset the drop indicator needs.
  const grab = useRef<{ x: number; y: number } | null>(null);
  const dragged = useRef(false);

  const onMouseDownCapture = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    dragged.current = false;
    const target = e.target as HTMLElement;
    if (target.closest(BAR_NO_DRAG_SELECTOR)) return;
    grab.current = { x: e.clientX, y: e.clientY };
  }, []);

  const onMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const start = grab.current;
      if (!start) return;
      if (Math.abs(e.clientX - start.x) + Math.abs(e.clientY - start.y) < threshold) return;
      grab.current = null;
      dragged.current = true;
      e.preventDefault();
      void armBarSnap(start);
      getCurrentWindow()
        .startDragging()
        .catch((error) => console.debug("barDrag: startDragging failed:", error));
    },
    [threshold],
  );

  const onMouseUp = useCallback(() => {
    grab.current = null;
  }, []);

  // Right-click: the webview's own menu is never right on the bar, so it is
  // suppressed and Rust pops the native one at the cursor. The drag arms on
  // the left button only, so this cannot interfere with it.
  const onContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    grab.current = null;
    void invoke(COMMANDS.BAR_SHOW_CONTEXT_MENU).catch((error) =>
      console.debug("barDrag: context menu failed:", error),
    );
  }, []);

  const swallowClickAfterDrag = useCallback((e: React.MouseEvent) => {
    if (!dragged.current) return false;
    dragged.current = false;
    e.stopPropagation();
    e.preventDefault();
    return true;
  }, []);

  // A drag that ends off the bar (a fast flick, a release outside the window)
  // never fires a click, so a window-level mouseup is the reliable settle
  // trigger. The settle itself is idempotent: whichever path fires first
  // disarms the other.
  useEffect(() => {
    const onUp = () => void settleBarSnap();
    window.addEventListener("mouseup", onUp, true);
    return () => window.removeEventListener("mouseup", onUp, true);
  }, []);

  useBarDisplayFollow(displayFollowPaused);

  return {
    dragProps: { onMouseDownCapture, onMouseMove, onMouseUp, onContextMenu },
    swallowClickAfterDrag,
  };
}
