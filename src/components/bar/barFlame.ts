import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS, UI } from "@/lib/constants.generated";

/**
 * The activity-indicator flame shared by the Pill and the Island: which
 * colour, how bright, and whether it breathes. `null` means no flame (idle).
 * The colour/intensity are a small language:
 *   blue  = Juno's own listening / working    green = your dictation input
 *   red   = error                             dim   = ambient, bright = engaged
 *   pulse = ongoing work (thinking / processing / ambient wait)
 * Named states win over the generic working fallback. TRANSCRIBING takes the
 * colour of whoever was listening: green after dictation, blue after a
 * request to Juno, so asking Juno never flashes the dictation colour between
 * the listening blue and the working blue.
 */
export function flameForState(
  state: string,
  isWorking: boolean,
  sessionColor: string,
  isDictation: boolean,
): { color: string; intensity: number; pulse: boolean } | null {
  switch (state) {
    case UI.BAR_STATES_DICTATING:
      return { color: "#30D158", intensity: 0.2, pulse: false }; // green: your speech -> text
    case UI.BAR_STATES_TRANSCRIBING:
      // breathe: converting your speech, in the colour it was heard in
      return { color: isDictation ? "#30D158" : "#0A84FF", intensity: 0.13, pulse: true };
    case UI.BAR_STATES_LISTENING:
      return { color: "#0A84FF", intensity: 0.2, pulse: false }; // blue: listening to your request
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { color: "#0A84FF", intensity: 0.07, pulse: true }; // dim blue breathe: ambient wait
    case UI.BAR_STATES_ERROR:
      return { color: "#FF453A", intensity: 0.24, pulse: false }; // red: something failed
    default:
      break;
  }
  if (isWorking) {
    // Agent working: the focused session's own colour, breathing.
    return { color: sessionColor, intensity: 0.22, pulse: true };
  }
  return null;
}

/**
 * Whether the glowing border shows at all. Read from the same bar config the
 * settings window writes, and kept current from the config-changed event so
 * the toggle takes effect without a restart. On by default.
 */
export function useShowGlowBorder(): boolean {
  const [show, setShow] = useState(true);
  useEffect(() => {
    let cancelled = false;
    void invoke<{ show_glow_border?: boolean }>(COMMANDS.BAR_UI_GET_BAR_CONFIG)
      .then((config) => {
        if (!cancelled && typeof config?.show_glow_border === "boolean") {
          setShow(config.show_glow_border);
        }
      })
      .catch((error) => console.debug("bar: could not read the glow-border setting:", error));
    return () => {
      cancelled = true;
    };
  }, []);
  useEventListener<{ show_glow_border?: boolean } | null>(EVENTS.BAR_CONFIG_CHANGED, (payload) => {
    if (typeof payload?.show_glow_border === "boolean") setShow(payload.show_glow_border);
  });
  return show;
}
