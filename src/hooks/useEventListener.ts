import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeCleanupEventListener } from '@/lib/safeEventCleanup';

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

    const setupListener = async () => {
      try {
        unlisten = await listen<T>(eventName, (event) => {
          savedHandler.current(event.payload);
        });
      } catch (error) {
        console.error(`Failed to setup listener for ${eventName}:`, error);
      }
    };

    setupListener();

    return () => {
      // Use safeCleanupEventListener for proper cleanup
      safeCleanupEventListener(unlisten);
    };
  }, [eventName, ...dependencies]);
}