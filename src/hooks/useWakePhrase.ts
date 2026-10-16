import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

export interface WakePhrase {
  /** A voice trigger exists: the person's standing intent. */
  configured: boolean;
  /** The engine is running right now: an outcome, never a guess. */
  listening: boolean;
  /** Pause or resume the engine. Never touches the trigger itself. */
  toggle: () => void;
}

/**
 * The wake phrase, as a bar shows it. Shared by the Pill and the Island.
 *
 * Two separate facts, both answered by Rust rather than inferred here.
 * `configured` is whether a voice trigger exists at all, which lives in the
 * triggers list. `listening` is whether the engine is actually running, which
 * can be false with a trigger enabled because the engine failed to start, or
 * because a bar paused it. A dot that claims Juno is listening when it is not
 * is worse than no dot, so neither is ever guessed.
 */
export function useWakePhrase(who = "Bar"): WakePhrase {
  const [configured, setConfigured] = useState(false);
  const [listening, setListening] = useState(false);

  // The startup arming happens while this webview is still loading, so the
  // first voice-trigger-listening event is usually gone before anyone is
  // listening for it. Ask once on mount for the same two facts.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [on, triggers] = await Promise.all([
          invoke<boolean>(COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_STATUS),
          invoke<Array<{ gesture?: string; enabled?: boolean; phrase?: string | null }>>(
            COMMANDS.TRIGGERS_GET_TRIGGERS,
          ),
        ]);
        if (cancelled) return;
        setListening(Boolean(on));
        setConfigured(
          (triggers ?? []).some(
            (t) => t.gesture === "say" && t.enabled === true && Boolean(t.phrase?.trim()),
          ),
        );
      } catch (error) {
        console.debug(`${who}: could not read the wake phrase state:`, error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [who]);

  // The one signal for both. Rust emits it whenever it arms or disarms the
  // engine: at startup, when the triggers change, when a bar pauses or
  // resumes it, and after a coordinated stop re-arms. It carries the outcome,
  // so an engine that failed to start reads as not listening even though the
  // trigger is there, which is the honest thing to draw.
  useEventListener<{ listening?: boolean; phrases?: string[] } | null>(
    EVENTS.VOICE_TRIGGER_LISTENING,
    (payload) => {
      setListening(Boolean(payload?.listening));
      setConfigured((payload?.phrases?.length ?? 0) > 0);
    },
  );

  /**
   * Pause or resume the wake phrase. This controls the engine, never the
   * trigger: pausing writes nothing to the triggers list, and the next launch
   * arms it again from the stored triggers. The bar does not move the dot
   * itself; both commands report what actually happened through the event
   * above, and the dot follows that.
   */
  const toggle = useCallback(() => {
    const command = listening
      ? COMMANDS.ALWAYS_LISTENING_STOP_ALWAYS_LISTENING
      : COMMANDS.ALWAYS_LISTENING_START_ALWAYS_LISTENING;
    void invoke(command).catch((error) =>
      console.error(`${who}: could not change the wake phrase engine:`, error),
    );
  }, [listening, who]);

  return { configured, listening, toggle };
}
