import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { isTauriAvailable, safeUnlisten } from '@/lib/tauri-event-utils';

export function useEventListener<T = any>(
  eventName: string,
  handler: (payload: T) => void,
  dependencies: any[] = []
) {
  const savedHandler = useRef(handler);

  // Update ref when handler changes
  useEffect(() => {
    savedHandler.current = handler;
  }, [handler]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    const setupListener = async () => {
      try {
        // Check if we're in a Tauri environment
        if (!isTauriAvailable()) {
          console.warn(`Cannot setup listener for ${eventName}: Not in Tauri environment`);
          return;
        }

        const unlistenFn = await listen<T>(eventName, (event) => {
          if (mounted) {
            savedHandler.current(event.payload);
          }
        });

        if (mounted) {
          unlisten = unlistenFn;
        } else {
          // If component unmounted during async setup, clean up immediately
          safeUnlisten(unlistenFn);
        }
      } catch (error) {
        console.error(`Failed to setup listener for ${eventName}:`, error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      safeUnlisten(unlisten);
    };
  }, [eventName, ...dependencies]);
}