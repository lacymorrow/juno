import { useCallback } from 'react';
import { Window, currentMonitor } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

interface WindowSizeConfig {
  width: number;
  height: number;
  /**
   * Logical y, inside the window, of the point that must stay put on screen
   * across the resize (the bar keeps its pill's vertical centre here, so a
   * pill that grows on hover grows around itself instead of sliding down).
   * Omitted: the top edge stays put.
   */
  anchorY?: number;
  /**
   * Grow upward instead of downward. When the bar is docked in the bottom half
   * of its display, its chat pane opens ABOVE the pill: the window's added
   * height must extend up while the pill's on-screen position is unchanged
   * (the bottom edge moves down as much as the top moves up only through the
   * `anchorY` maths — the pill itself stays put). Omitted/false keeps today's
   * downward growth.
   */
  growUp?: boolean;
}

// Cache last applied sizes per window to avoid redundant resizes
const lastSizeByLabel: Map<string, { width: number; height: number }> = new Map();

/** What the last resize left behind, so the next one can keep the pill fixed. */
interface AnchorState {
  /** Physical height applied last time. */
  physH: number;
  /** Logical anchorY passed last time (pill centre offset from the near edge). */
  anchor: number;
  /** Whether that resize grew upward. */
  growUp: boolean;
}
// Last anchor state per window, so the next resize knows where the pill was.
const lastAnchorByLabel: Map<string, AnchorState> = new Map();

/**
 * The new physical top edge for a resize that keeps the pill's vertical centre
 * at the same screen position, whichever direction the window grows.
 *
 * `anchor` is the pill centre's distance from the window's *near* edge (top when
 * growing down, bottom when growing up), so the pill centre's distance from the
 * top is `anchor` (down) or `physH - anchor` (up). Keeping `top + that distance`
 * constant across the resize both fixes the pill and moves the top up when a
 * growUp window gains height. Falls back to a top-anchored resize when there is
 * no previous state or no anchor (matching the original downward behaviour).
 */
export function anchoredTop(
  prevTop: number,
  prev: AnchorState | undefined,
  next: { physH: number; anchor?: number; growUp?: boolean },
  scale: number,
): number {
  if (prev === undefined || next.anchor === undefined) return prevTop;
  const fromTop = (physH: number, anchor: number, growUp: boolean) =>
    growUp ? physH - Math.round(anchor * scale) : Math.round(anchor * scale);
  const fOld = fromTop(prev.physH, prev.anchor, prev.growUp);
  const fNew = fromTop(next.physH, next.anchor, next.growUp ?? false);
  return prevTop + fOld - fNew;
}

/**
 * Center-stable resize: adjusts the window X position so the horizontal center
 * stays constant. Without this, macOS resizes from the top-left anchor, causing
 * the centered island to jump horizontally.
 *
 * Vertical: anchored on `anchorY` when given (the previous call's anchor stays
 * at the same screen position), otherwise top-anchored — the window
 * grows/shrinks downward.
 *
 * Uses physical pixel coordinates to match outerPosition()/outerSize() units.
 */
async function centerStableResize(appWindow: Window, next: WindowSizeConfig) {
  const scaleFactor = await appWindow.scaleFactor();
  const physNextW = Math.round(next.width * scaleFactor);
  const physNextH = Math.round(next.height * scaleFactor);

  const pos = await appWindow.outerPosition();   // PhysicalPosition
  const size = await appWindow.outerSize();       // PhysicalSize

  const dx = physNextW - size.width;
  const newX = dx !== 0 ? Math.round(pos.x - dx / 2) : pos.x;

  const prevAnchor = lastAnchorByLabel.get(appWindow.label);
  const anchoredY = anchoredTop(
    pos.y,
    prevAnchor,
    { physH: physNextH, anchor: next.anchorY, growUp: next.growUp },
    scaleFactor,
  );
  if (next.anchorY !== undefined) {
    lastAnchorByLabel.set(appWindow.label, {
      physH: physNextH,
      anchor: next.anchorY,
      growUp: next.growUp ?? false,
    });
  }

  const clamped = await clampToMonitor(newX, anchoredY, physNextW, physNextH);
  const clampedX = clamped.x;
  const newY = clamped.y;

  // Apply the move + resize atomically through the backend (a single macOS
  // NSWindow setFrame:). Issuing setPosition and setSize separately let the
  // WindowServer composite them in different frames, so for one frame the
  // window showed its new width still anchored at the old top-left and the
  // centered pill/dot visibly jumped before snapping back. One transaction
  // removes that seam. (Off macOS the command falls back to separate setters.)
  await invoke("set_bar_frame", {
    x: clampedX,
    y: newY,
    width: next.width,
    height: next.height,
  });
}

/**
 * Keep a resized window on its current monitor. A window growing downward (the
 * bar opening its chat pane) must not run off the bottom; an edge-docked bar
 * whose pane widens must not run off the left or right. Each axis is left
 * untouched when it already fits, otherwise nudged just enough to fit, never
 * past the monitor's opposite edge. Falls back to the input position when
 * monitor info is unavailable (tests, headless).
 */
async function clampToMonitor(
  x: number,
  y: number,
  physW: number,
  physH: number,
): Promise<{ x: number; y: number }> {
  try {
    const monitor = await currentMonitor();
    if (!monitor) return { x, y };
    const { position, size } = monitor;
    const right = position.x + size.width;
    const bottom = position.y + size.height;
    const clampedX =
      x + physW <= right ? Math.max(position.x, x) : Math.max(position.x, right - physW);
    const clampedY =
      y + physH <= bottom ? Math.max(position.y, y) : Math.max(position.y, bottom - physH);
    return { x: clampedX, y: clampedY };
  } catch {
    return { x, y };
  }
}

export function useWindowSize(windowLabel: string) {
  const resizeWindow = useCallback(async (config: WindowSizeConfig) => {
    try {
      const appWindow = await Window.getByLabel(windowLabel);
      if (appWindow) {
        await centerStableResize(appWindow, config);
      }
    } catch (error) {
      console.error(`Failed to resize window ${windowLabel}:`, error);
    }
  }, [windowLabel]);

  const resizeWindowIfChanged = useCallback(async (config: WindowSizeConfig) => {
    try {
      const prev = lastSizeByLabel.get(windowLabel);
      if (prev && prev.width === config.width && prev.height === config.height) {
        return; // no-op
      }

      const appWindow = await Window.getByLabel(windowLabel);
      if (appWindow) {
        await centerStableResize(appWindow, config);
        lastSizeByLabel.set(windowLabel, { width: config.width, height: config.height });
      }
    } catch (error) {
      console.error(`Failed to resize window ${windowLabel}:`, error);
    }
  }, [windowLabel]);

  const getWindowSize = useCallback(async (): Promise<WindowSizeConfig | null> => {
    try {
      const appWindow = await Window.getByLabel(windowLabel);
      if (appWindow) {
        const size = await appWindow.innerSize();
        return {
          width: size.width,
          height: size.height,
        };
      }
      return null;
    } catch (error) {
      console.error(`Failed to get window size for ${windowLabel}:`, error);
      return null;
    }
  }, [windowLabel]);

  return {
    resizeWindow,
    resizeWindowIfChanged,
    getWindowSize,
  };
}
