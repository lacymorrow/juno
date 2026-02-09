/**
 * Safe event cleanup utility that handles cases where Tauri context might not be available
 */
export function safeCleanupEventListener(unlisten: (() => void) | undefined | null) {
  if (!unlisten || typeof unlisten !== 'function') {
    return;
  }

  // Wrap in setTimeout to ensure cleanup happens after current execution context
  setTimeout(() => {
    try {
      // Check if we're in a Tauri context and the window is still available
      if (
        typeof window !== 'undefined' && 
        window.__TAURI_EVENT_PLUGIN_INTERNALS__ &&
        typeof window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener === 'function'
      ) {
        unlisten();
      } else {
        // If Tauri internals aren't available, we're likely in a cleanup phase
        // where the window is already destroyed or we're not in a Tauri context
        console.debug('Skipping event cleanup - Tauri context not available');
      }
    } catch (error) {
      // Silently ignore cleanup errors - this is expected during window destruction
      console.debug('Event cleanup error (expected during shutdown):', error);
    }
  }, 0);
}

/**
 * Creates a safe event listener setup that handles cleanup properly.
 * Returns a cleanup function that safely tears down the listener.
 */
export function createSafeEventListener<T>(
  eventName: string,
  handler: (event: { payload: T }) => void
): () => void {
  let unlisten: (() => void) | undefined;
  let disposed = false;

  const setupListener = async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      const fn = await listen<T>(eventName, (event) => {
        if (!disposed) handler(event);
      });
      if (disposed) {
        // Already cleaned up before listen() resolved
        safeCleanupEventListener(fn);
      } else {
        unlisten = fn;
      }
    } catch (error) {
      console.error(`Failed to setup listener for ${eventName}:`, error);
    }
  };

  setupListener();

  return () => {
    disposed = true;
    safeCleanupEventListener(unlisten);
  };
}