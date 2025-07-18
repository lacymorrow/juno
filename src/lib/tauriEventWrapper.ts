import { listen as tauriListen, type EventCallback, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * Wrapper around Tauri's listen function that provides safe cleanup
 */
export async function safeListen<T>(
  event: string,
  handler: EventCallback<T>
): Promise<UnlistenFn> {
  let isListening = true;
  let unlistenFn: UnlistenFn | null = null;

  try {
    // Create the listener
    unlistenFn = await tauriListen<T>(event, (e) => {
      if (isListening) {
        handler(e);
      }
    });

    // Return a safe unlisten function
    return () => {
      isListening = false;
      
      // Defer cleanup to next tick
      setTimeout(() => {
        if (unlistenFn) {
          try {
            // Check if Tauri is still available
            if (
              typeof window !== 'undefined' && 
              window.__TAURI_EVENT_PLUGIN_INTERNALS__ &&
              typeof window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener === 'function'
            ) {
              unlistenFn();
            }
          } catch (error) {
            // Ignore cleanup errors
            console.debug('Event cleanup handled gracefully');
          }
          unlistenFn = null;
        }
      }, 0);
    };
  } catch (error) {
    console.error('Failed to setup event listener:', error);
    // Return a no-op function if setup fails
    return () => {};
  }
}

/**
 * Hook-friendly version that handles React component lifecycle
 */
export function useSafeTauriEvent<T>(
  event: string,
  handler: EventCallback<T>,
  deps: React.DependencyList = []
): void {
  const React = require('react');
  const { useEffect, useRef } = React;
  
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    const setup = async () => {
      try {
        unlisten = await safeListen<T>(event, (e) => {
          if (mounted) {
            handlerRef.current(e);
          }
        });
      } catch (error) {
        console.error(`Failed to setup listener for ${event}:`, error);
      }
    };

    setup();

    return () => {
      mounted = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [event, ...deps]);
}