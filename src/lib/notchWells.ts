/**
 * Wells and shapes for the Notch look.
 *
 * The Notch lives in the hardware: flush with the top of the display, centred
 * on the camera housing, the same black, so at rest it simply is the notch.
 * Dropped in a corner it becomes the corner of the screen peeled back like a
 * page, with Juno underneath. Those are its only homes: the notch and the four
 * corners. There is no centre well and no edge-midpoint; a notch in the middle
 * of the screen, or halfway down its side, is not a notch.
 *
 * Every well is FLUSH: no margin, no gap under the menu bar. The bar is lifted
 * over the menu bar while this look rests (`bar_stacking::OVER_MENU_BAR_LEVEL`
 * in Rust), so AppKit lets it sit at the very top edge.
 *
 * Pure, and in global desktop points like `snapWells.ts`.
 */

import type { MonitorRect, Well, WellSlot } from "./snapWells";

/** Which of its two forms the Notch takes in a well. */
export type NotchForm = "notch" | "curl";

/** Which screen corner a curl is peeled back from. */
export type CurlCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

/**
 * The resting notch's width, in points. Wider than the camera housing on
 * every notched MacBook (about 185 to 200 points) so the shape covers it whole
 * and leaves a small ear either side, where the status dot shows. On a display
 * without a notch it reads as a small black tab at the top.
 */
export const NOTCH_REST_WIDTH = 236;

/**
 * The strip the shape draws over the menu bar when the display reports none
 * (the preview, a menu bar set to hide). A standard menu bar is 24 points; a
 * notched one is 37 or 38.
 */
export const FALLBACK_MENU_BAR = 32;

/** The resting curl: the corner peeled back this far along each edge, in points. */
export const CURL_REST = 44;

/** The slot each well is: top centre, and the four corners. */
export const NOTCH_SLOTS: readonly WellSlot[] = [
  { fx: 0.5, fy: 0 },
  { fx: 0, fy: 0 },
  { fx: 1, fy: 0 },
  { fx: 0, fy: 1 },
  { fx: 1, fy: 1 },
];

/** The notch form sits at top centre; every other well is a corner. */
export function notchFormFor(slot: WellSlot | null): NotchForm {
  if (!slot) return "notch";
  return slot.fx === 0.5 && slot.fy === 0 ? "notch" : "curl";
}

/** The corner a curl well peels from. */
export function curlCornerFor(slot: WellSlot): CurlCorner {
  const v = slot.fy >= 0.5 ? "bottom" : "top";
  const h = slot.fx >= 0.5 ? "right" : "left";
  return `${v}-${h}`;
}

/**
 * How tall the display's menu bar is, in points: the strip the shape covers
 * before any content can show. 37 or 38 on a notched MacBook, 24 elsewhere,
 * the fallback when the display reports no work area or a hidden menu bar.
 */
export function menuBarHeight(m: MonitorRect): number {
  const wa = m.workArea;
  if (!wa) return FALLBACK_MENU_BAR;
  const h = wa.position.y - m.position.y;
  return h > 0 ? h : FALLBACK_MENU_BAR;
}

/** The resting footprint for a slot on a display. */
export function notchRestSize(slot: WellSlot, m: MonitorRect): { width: number; height: number } {
  return notchFormFor(slot) === "notch"
    ? { width: NOTCH_REST_WIDTH, height: menuBarHeight(m) }
    : { width: CURL_REST, height: CURL_REST };
}

/**
 * The Notch's wells on every display: top centre and the four corners, each
 * flush with the display's own edges, sized for the resting shape it lands
 * as. Nothing else, and never the centre.
 */
export function notchWells(monitors: MonitorRect[]): Well[] {
  const wells: Well[] = [];
  monitors.forEach((m, monitorIndex) => {
    for (const slot of NOTCH_SLOTS) {
      const { width, height } = notchRestSize(slot, m);
      const x =
        slot.fx === 0
          ? m.position.x
          : slot.fx === 1
            ? m.position.x + m.size.width - width
            : Math.round(m.position.x + (m.size.width - width) / 2);
      const y = slot.fy === 0 ? m.position.y : m.position.y + m.size.height - height;
      wells.push({ x, y, width, height, fx: slot.fx, fy: slot.fy, monitorIndex });
    }
  });
  return wells;
}

/**
 * The well nearest a dragged shape, judged by centres. The Notch's wells are
 * different sizes (a wide notch, small square corners), so comparing top-left
 * corners would favour whichever well happened to share the grab's corner.
 * `origin` and `size` are the dragged footprint's top-left and size.
 */
export function nearestNotchWell(
  origin: { x: number; y: number },
  size: { width: number; height: number },
  wells: Well[],
): Well | null {
  const cx = origin.x + size.width / 2;
  const cy = origin.y + size.height / 2;
  let best: Well | null = null;
  let bestD = Infinity;
  for (const w of wells) {
    const dx = w.x + w.width / 2 - cx;
    const dy = w.y + w.height / 2 - cy;
    const d = dx * dx + dy * dy;
    if (d < bestD) {
      bestD = d;
      best = w;
    }
  }
  return best;
}

/** CSS transform that mirrors a top-left drawing into `corner`. */
export function curlMirror(corner: CurlCorner): string {
  const sx = corner.endsWith("right") ? -1 : 1;
  const sy = corner.startsWith("bottom") ? -1 : 1;
  return `scale(${sx}, ${sy})`;
}
