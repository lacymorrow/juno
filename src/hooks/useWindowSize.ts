import { useCallback } from 'react';
import { LogicalSize, PhysicalPosition, Window } from '@tauri-apps/api/window';

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

  const pos = await appWindow.outerPosition();   // PhysicalPosition
  const size = await appWindow.outerSize();       // PhysicalSize

  const dx = physNextW - size.width;
  if (dx !== 0) {
    const newX = Math.round(pos.x - dx / 2);
    await Promise.all([
      appWindow.setPosition(new PhysicalPosition(newX, pos.y)),
      appWindow.setSize(new LogicalSize(next.width, next.height)),
    ]);
  } else {
    await appWindow.setSize(new LogicalSize(next.width, next.height));
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
