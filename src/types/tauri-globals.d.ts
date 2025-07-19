/**
 * Global type declarations for Tauri window properties
 */

declare global {
  interface Window {
    __TAURI__?: {
      [key: string]: any;
    };
    __TAURI_EVENT_PLUGIN_INTERNALS__?: {
      unregisterListener?: (id: number) => void;
      [key: string]: any;
    };
  }
}

export {};