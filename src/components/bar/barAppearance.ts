import { UI, WINDOW_LABELS } from "@/lib/constants.generated";

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


