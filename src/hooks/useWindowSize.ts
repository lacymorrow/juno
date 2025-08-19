import { useCallback } from 'react';
import { LogicalSize, Window } from '@tauri-apps/api/window';

interface WindowSizeConfig {
  width: number;
  height: number;
}

// Cache last applied sizes per window to avoid redundant resizes
const lastSizeByLabel: Map<string, { width: number; height: number }> = new Map();

export function useWindowSize(windowLabel: string) {
  const resizeWindow = useCallback(async (config: WindowSizeConfig) => {
    try {
      const appWindow = await Window.getByLabel(windowLabel);
      if (appWindow) {
        await appWindow.setSize(new LogicalSize(config.width, config.height));
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
        await appWindow.setSize(new LogicalSize(config.width, config.height));
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
