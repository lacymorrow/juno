// Tauri window internals type declaration
declare global {
  interface Window {
    __TAURI_EVENT_PLUGIN_INTERNALS__?: {
      unregisterListener?: (id: number) => void;
      [key: string]: any;
    };
  }
}

export {};