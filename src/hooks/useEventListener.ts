import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeCleanupEventListener } from '@/lib/safeEventCleanup';

export function useEventListener<T = any>(
  eventName: string,
  handler: (payload: T) => void,
  dependencies: any[] = []
) {
  const savedHandler = useRef(handler);

  // Update ref when handler changes — ensures listener always calls latest handler
  useEffect(() => {
    savedHandler.current = handler;
  }, [handler]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    const setupListener = async () => {
      try {
        const unlistenFn = await listen<T>(eventName, (event) => {
          if (mounted) {
            savedHandler.current(event.payload);
          }
        });
        if (mounted) {
          unlisten = unlistenFn;
        } else {
          // Component unmounted before listen() resolved — clean up immediately
          safeCleanupEventListener(unlistenFn);
        }
      } catch (error) {
        console.error(`Failed to setup listener for ${eventName}:`, error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, [eventName, ...dependencies]);
}