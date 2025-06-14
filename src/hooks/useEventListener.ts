import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';

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
      if (unlisten) {
        unlisten();
      }
    };
  }, [eventName, ...dependencies]);
}