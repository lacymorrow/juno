import { isIdleState, isInputState, isVoiceState } from "../island/islandModel";

/**
 * The Notch's postures: the shapes it takes, decided from the bar state and
 * whether a card is open.
 *
 * These used to be the Island's too. The Island became one surface with four
 * faces (`../island/islandModel.ts`, docs/plans/island-appearance.md); the
 * Notch keeps the six postures until it gets the same pass, so nothing about
 * it changed when the Island did.
 */

/** The six shapes the notch takes. */
export type Posture = "capsule" | "hover" | "ear" | "line" | "status" | "card";

/** Card height follows content, in these steps. */
export const CARD_HEIGHT_STEP = 8;

export interface PostureSize {
  width: number;
  height: number;
  radius: number;
}

/** Sizes per posture. Ear and status match on purpose: the shape must not
 *  lurch wider the instant someone stops talking. */
export const POSTURE_SIZES: {
  capsule: PostureSize;
  hover: PostureSize;
  ear: PostureSize;
  line: PostureSize;
  status: PostureSize;
  card: { width: number; minHeight: number; maxHeight: number; radius: number };
} = {
  capsule: { width: 92, height: 28, radius: 14 },
  hover: { width: 132, height: 32, radius: 16 },
  ear: { width: 300, height: 32, radius: 16 },
  line: { width: 340, height: 36, radius: 18 },
  status: { width: 300, height: 32, radius: 16 },
  card: { width: 360, minHeight: 96, maxHeight: 320, radius: 20 },
};

export interface PostureInput {
  state: string;
  /** A response card is on stage (answer, approval, or error inside a card). */
  cardOpen: boolean;
  /** Juno holds the physical cursor; the shape has to say so in words. */
  driving?: boolean;
  /** The pointer is over the shape and it may show its controls. Only an idle shape answers it. */
  hovered?: boolean;
}

/** Which posture a combination of backend state and local state gets. */
export function posture({ state, cardOpen, driving = false, hovered = false }: PostureInput): Posture {
  if (cardOpen) return "card";
  if (driving) return "status";
  if (isInputState(state)) return "line";
  if (isVoiceState(state)) return "ear";
  if (isIdleState(state)) return hovered ? "hover" : "capsule";
  return "status";
}

/** Clamp a measured card content height to the card's range, in steps. */
export function cardHeightFor(contentHeight: number): number {
  const { minHeight, maxHeight } = POSTURE_SIZES.card;
  const stepped = Math.ceil(contentHeight / CARD_HEIGHT_STEP) * CARD_HEIGHT_STEP;
  return Math.min(maxHeight, Math.max(minHeight, stepped));
}

/** The shape's own size for a posture. */
export function postureSize(p: Posture, cardHeight = POSTURE_SIZES.card.minHeight): PostureSize {
  if (p === "card") {
    return {
      width: POSTURE_SIZES.card.width,
      height: cardHeightFor(cardHeight),
      radius: POSTURE_SIZES.card.radius,
    };
  }
  return { ...POSTURE_SIZES[p] };
}

export interface RingInput {
  cardOpen: boolean;
  streaming: boolean;
  working: boolean;
  approvalPending: boolean;
}

/** Whether the linger ring should be counting down at all. */
export function ringShouldRun({ cardOpen, streaming, working, approvalPending }: RingInput): boolean {
  return cardOpen && !streaming && !working && !approvalPending;
}
