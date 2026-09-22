import { UI, WINDOW_LABELS } from "@/lib/constants.generated";

/**
 * Always-on depth glow for the dark bar and chat pane. A black surface on a
 * black window (dark wallpaper, a dark app behind it) disappears; this gives it
 * a faint light rim plus a soft bloom so it separates on any background, and a
 * dark elevation so it still lifts on light ones. Static CSS, so it costs no
 * GPU at idle (the flame border still carries live activity). A little depth,
 * not a light source.
 */
export const BAR_DEPTH_GLOW =
  "0 0 0 0.5px rgba(255,255,255,0.16), 0 0 18px 1px rgba(255,255,255,0.06), 0 10px 30px rgba(0,0,0,0.5)";

export type BarAppearance =
  | typeof UI.BAR_APPEARANCES_FLOATING
  | typeof UI.BAR_APPEARANCES_APP
  | typeof UI.BAR_APPEARANCES_VOICE_AI
  | typeof UI.BAR_APPEARANCES_DYNAMIC
  | typeof UI.BAR_APPEARANCES_ORB
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


