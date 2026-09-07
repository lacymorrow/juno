import { useCallback } from 'react';
import { LogicalSize, PhysicalPosition, Window, currentMonitor } from "@tauri-apps/api/window";

interface WindowSizeConfig {
  width: number;
  height: number;
}

// Cache last applied sizes per window to avoid redundant resizes
const lastSizeByLabel: Map<string, { width: number; height: number }> = new Map();

/**
 * Center-stable resize: adjusts the window X position so the horizontal center
 * stays constant. Without this, macOS resizes from the top-left anchor, causing
 * the centered island to jump horizontally.
 *
 * Vertical: top-anchored — the window grows/shrinks downward. The island is
 * positioned at the top of its container so it stays visually stable.
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
  const newY = await clampedTop(pos.y, physNextH);

  if (newX !== pos.x || newY !== pos.y) {
    await Promise.all([
      appWindow.setPosition(new PhysicalPosition(newX, newY)),
      appWindow.setSize(new LogicalSize(next.width, next.height)),
    ]);
  } else {
    await appWindow.setSize(new LogicalSize(next.width, next.height));
  }
}

/**
 * A window that grows downward (the bar opening its chat pane) must not run
 * off the bottom of the screen. Returns the top edge to use: unchanged when
 * the new height fits, otherwise moved up just enough to fit, never above the
 * monitor's top. Falls back to the current position when monitor info is
 * unavailable (tests, headless).
 */
async function clampedTop(currentY: number, physNextH: number): Promise<number> {
  try {
    const monitor = await currentMonitor();
    if (!monitor) return currentY;
    const bottom = monitor.position.y + monitor.size.height;
    if (currentY + physNextH <= bottom) return currentY;
    return Math.max(monitor.position.y, bottom - physNextH);
  } catch {
    return currentY;
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
