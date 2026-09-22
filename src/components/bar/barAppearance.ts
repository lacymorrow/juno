import { UI, WINDOW_LABELS } from "@/lib/constants.generated";

/**
 * Always-on separation glow for the dark bar and chat pane. A black surface on
 * a black window (dark wallpaper, a dark app behind it) disappears; this draws
 * a thin WHITE rim hugging the edge plus a very tight white bloom so it stands
 * off ANY background including pure black, with a small dark drop so it still
 * lifts on light ones. Static CSS, so it costs no GPU at idle and is fully
 * independent of the flame border (which only mounts while the bar is active).
 *
 * Sized to stay INSIDE the window frame: the bar window is `overflow-hidden`
 * and only pad px larger than the pill on each side (pad = 16 for
 * compact/hover/voice, 24 for full — see BAR_LAYOUTS), so the tightest room
 * before the window edge clips the halo is ~16px. Every offset+blur here stays
 * under ~10px, well within that, so nothing is clipped. A couple px of
 * transparent window padding would let a softer halo breathe; that is a window
 * sizing change, not made here. Keep this a single swappable value.
 */
export const BAR_DEPTH_GLOW =
  "0 0 0 0.5px rgba(255,255,255,0.5), 0 0 4px rgba(255,255,255,0.28), 0 2px 8px rgba(0,0,0,0.5)";

export type BarAppearance =
  | typeof UI.BAR_APPEARANCES_FLOATING
  | typeof UI.BAR_APPEARANCES_APP
  | typeof UI.BAR_APPEARANCES_VOICE_AI
  | typeof UI.BAR_APPEARANCES_DYNAMIC
  | typeof UI.BAR_APPEARANCES_ORB
  | typeof UI.BAR_APPEARANCES_REACT_ORB
  | typeof UI.BAR_APPEARANCES_PERSONA;

export function getBarLayoutWindowLabel(appearance: BarAppearance): string {
  switch (appearance) {
    case UI.BAR_APPEARANCES_APP:
      return WINDOW_LABELS.APP_BAR;
    case UI.BAR_APPEARANCES_VOICE_AI:
      return WINDOW_LABELS.VOICE_BAR;
    case UI.BAR_APPEARANCES_DYNAMIC:
      return WINDOW_LABELS.DYNAMIC_BAR;
    case UI.BAR_APPEARANCES_FLOATING:
    default:
      return WINDOW_LABELS.FLOATING_BAR;
  }
}


