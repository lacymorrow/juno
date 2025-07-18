import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

interface ShortcutEventPayload {
  state: "pressed" | "released";
  shortcut: string;
  test_mode?: boolean;
}

interface UseShortcutEventsProps {
  onAgentModeShortcut?: (payload: ShortcutEventPayload) => void;
  onDictationInputShortcut?: (payload: ShortcutEventPayload) => void;
}

/**
 * Hook to listen for global shortcut events from the backend
 * This ensures consistent event handling across the application
 */
export function useShortcutEvents({
  onAgentModeShortcut,
  onDictationInputShortcut,
}: UseShortcutEventsProps) {
  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      // Listen for agent mode shortcut events
      if (onAgentModeShortcut) {
        const unlisten = await listen<ShortcutEventPayload>(
          EVENTS.SHORTCUTS_AGENT_MODE,
          (event) => {
            console.log("[Shortcut Event] Agent mode:", event.payload);
            onAgentModeShortcut(event.payload);
          }
        );
        unlisteners.push(unlisten);
      }

      // Listen for dictation input shortcut events
      if (onDictationInputShortcut) {
        const unlisten = await listen<ShortcutEventPayload>(
          EVENTS.SHORTCUTS_DICTATION_INPUT,
          (event) => {
            console.log("[Shortcut Event] Dictation input:", event.payload);
            onDictationInputShortcut(event.payload);
          }
        );
        unlisteners.push(unlisten);
      }
    };

    setupListeners().catch((error) => {
      console.error("[Shortcut Events] Failed to setup listeners:", error);
    });

    // Cleanup
    return () => {
      unlisteners.forEach((unlisten) => {
        safeCleanupEventListener(unlisten);
      });
    };
  }, [onAgentModeShortcut, onDictationInputShortcut]);
}