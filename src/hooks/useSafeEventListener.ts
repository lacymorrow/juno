import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeUnlisten } from '../lib/tauri-event-utils';

/**
 * Safe event listener hook that handles cleanup gracefully
 * Prevents errors when window.__TAURI_EVENT_PLUGIN_INTERNALS__ is undefined
 */
export function useSafeEventListener<T = any>(
  eventName: string,
  handler: (payload: T) => void,
  dependencies: any[] = []
) {
  const savedHandler = useRef(handler);
  const unlistenRef = useRef<(() => void) | null>(null);

  // Update ref when handler changes
  useEffect(() => {
    savedHandler.current = handler;
  }, [handler]);

  useEffect(() => {
    let mounted = true;

    const setupListener = async () => {
      try {
        // Check if we're in a Tauri environment
        if (typeof window === 'undefined' || !window.__TAURI__) {
          // Use debug level instead of warn - this is expected in non-Tauri environments
          console.debug(`Cannot setup listener for ${eventName}: Not in Tauri environment`);
          return;
        }

        const unlisten = await listen<T>(eventName, (event) => {
          if (mounted) {
            savedHandler.current(event.payload);
          }
        });

        if (mounted) {
          unlistenRef.current = unlisten;
        } else {
          // If component unmounted during async setup, clean up immediately
          safeUnlisten(unlisten);
        }
      } catch (error) {
        console.error(`Failed to setup listener for ${eventName}:`, error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      
      // Safe cleanup using utility
      if (unlistenRef.current) {
        safeUnlisten(unlistenRef.current);
        unlistenRef.current = null;
      }
    };
  }, [eventName, ...dependencies]);
}

// Re-export for backward compatibility
export { useSafeEventListener as useEventListener };