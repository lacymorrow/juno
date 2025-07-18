import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

interface DictationStateChangeEvent {
  previous_state: string;
  new_state: string;
  timestamp: number;
  reason: string;
  component: string;
}

interface UseDictationStateEventsProps {
  onStateChanged?: (event: DictationStateChangeEvent) => void;
  onForceReset?: (reason: string) => void;
  onInputChanged?: (state: any) => void;
}

/**
 * Hook to listen for dictation state management events from the backend
 * This ensures UI stays in sync with the backend dictation state
 */
export function useDictationStateEvents({
  onStateChanged,
  onForceReset,
  onInputChanged,
}: UseDictationStateEventsProps) {
  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      // Listen for dictation state changes
      if (onStateChanged) {
        const unlisten = await listen<DictationStateChangeEvent>(
          EVENTS.DICTATION_STATE_CHANGED,
          (event) => {
            console.log("[Dictation State] State changed:", event.payload);
            onStateChanged(event.payload);
          }
        );
        unlisteners.push(unlisten);
      }

      // Listen for force reset events
      if (onForceReset) {
        const unlisten = await listen<string>(
          EVENTS.DICTATION_STATE_FORCE_RESET,
          (event) => {
            console.log("[Dictation State] Force reset:", event.payload);
            onForceReset(event.payload);
          }
        );
        unlisteners.push(unlisten);
      }

      // Listen for input state changes
      if (onInputChanged) {
        const unlisten = await listen<any>(
          EVENTS.DICTATION_STATE_INPUT_CHANGED,
          (event) => {
            console.log("[Dictation State] Input changed:", event.payload);
            onInputChanged(event.payload);
          }
        );
        unlisteners.push(unlisten);
      }
    };

    setupListeners().catch((error) => {
      console.error("[Dictation State Events] Failed to setup listeners:", error);
    });

    // Cleanup
    return () => {
      unlisteners.forEach((unlisten) => {
        safeCleanupEventListener(unlisten);
      });
    };
  }, [onStateChanged, onForceReset, onInputChanged]);
}