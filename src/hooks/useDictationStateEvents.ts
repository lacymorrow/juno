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
    let mounted = true;
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      // Listen for dictation state changes
      if (onStateChanged) {
        const unlisten = await listen<DictationStateChangeEvent>(
          EVENTS.DICTATION_STATE_CHANGED,
          (event) => {
            if (mounted) {
              console.log("[Dictation State] State changed:", event.payload);
              onStateChanged(event.payload);
            }
          }
        );
        if (mounted) unlisteners.push(unlisten);
        else safeCleanupEventListener(unlisten);
      }

      // Listen for force reset events
      if (onForceReset) {
        const unlisten = await listen<string>(
          EVENTS.DICTATION_STATE_FORCE_RESET,
          (event) => {
            if (mounted) {
              console.log("[Dictation State] Force reset:", event.payload);
              onForceReset(event.payload);
            }
          }
        );
        if (mounted) unlisteners.push(unlisten);
        else safeCleanupEventListener(unlisten);
      }

      // Listen for input state changes
      if (onInputChanged) {
        const unlisten = await listen<any>(
          EVENTS.DICTATION_STATE_INPUT_CHANGED,
          (event) => {
            if (mounted) {
              console.log("[Dictation State] Input changed:", event.payload);
              onInputChanged(event.payload);
            }
          }
        );
        if (mounted) unlisteners.push(unlisten);
        else safeCleanupEventListener(unlisten);
      }
    };

    setupListeners().catch((error) => {
      console.error("[Dictation State Events] Failed to setup listeners:", error);
    });

    // Cleanup
    return () => {
      mounted = false;
      unlisteners.forEach((unlisten) => {
        safeCleanupEventListener(unlisten);
      });
    };
  }, [onStateChanged, onForceReset, onInputChanged]);
}