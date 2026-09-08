import { useCallback } from 'react';
import { LogicalSize, PhysicalPosition, Window, currentMonitor } from "@tauri-apps/api/window";

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
}

// Cache last applied sizes per window to avoid redundant resizes
const lastSizeByLabel: Map<string, { width: number; height: number }> = new Map();
// Last anchor per window, so the next resize knows where the anchor was.
const lastAnchorByLabel: Map<string, number> = new Map();

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
  const anchorShift =
    next.anchorY !== undefined && prevAnchor !== undefined
      ? Math.round((prevAnchor - next.anchorY) * scaleFactor)
      : 0;
  if (next.anchorY !== undefined) lastAnchorByLabel.set(appWindow.label, next.anchorY);

  const clamped = await clampToMonitor(newX, pos.y + anchorShift, physNextW, physNextH);
  const clampedX = clamped.x;
  const newY = clamped.y;

  if (clampedX !== pos.x || newY !== pos.y) {
    await Promise.all([
      appWindow.setPosition(new PhysicalPosition(clampedX, newY)),
      appWindow.setSize(new LogicalSize(next.width, next.height)),
    ]);
  } else {
    await appWindow.setSize(new LogicalSize(next.width, next.height));
  }
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
