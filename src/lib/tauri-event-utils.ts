/**
 * Utility functions for safe Tauri event handling
 */

// Check if we're in a valid Tauri environment
export function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && 
         window.__TAURI__ !== undefined &&
         window.__TAURI__ !== null;
}

// Check if Tauri event plugin internals are available
export function isTauriEventInternalsAvailable(): boolean {
  return isTauriAvailable() && 
         typeof window.__TAURI_EVENT_PLUGIN_INTERNALS__ !== 'undefined' &&
         window.__TAURI_EVENT_PLUGIN_INTERNALS__ !== null;
}

// Safe unlisten function that handles missing Tauri context
export function safeUnlisten(unlisten: (() => void) | undefined | null): void {
  if (!unlisten) return;
  
  try {
    // Check if we can safely call unlisten
    if (isTauriEventInternalsAvailable()) {
      unlisten();
    } else {
      // If Tauri internals are not available, we can't unlisten
      // This might happen during window unload or in non-Tauri contexts
      console.debug('Tauri event internals not available for cleanup');
    }
  } catch (error) {
    // Silently handle cleanup errors
    // These often occur during window destruction and are safe to ignore
    console.debug('Safe unlisten caught error (safe to ignore):', error);
  }
}

// Batch unlisten helper for multiple listeners
export function safeUnlistenAll(unlisteners: ((() => void) | undefined | null)[]): void {
  unlisteners.forEach(safeUnlisten);
}