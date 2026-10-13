/**
 * Wells and shapes for the Notch look.
 *
 * The Notch lives on the screen's edges. At top centre it is the hardware
 * notch: flush with the top of the display, centred on the camera housing, the
 * same black, so at rest it simply is the notch. On any other edge stop (the
 * bottom, the left and right sides, and the quarter points of a wide display)
 * it is the same kind of tab growing out of that edge. In a corner it becomes
 * the corner of the screen peeled back like a page, with Juno underneath.
 *
 * The one place it never goes is the middle of the screen: a notch has to
 * grow out of an edge. So the wells are the grid's perimeter and nothing
 * inside it.
 *
 * Every well is FLUSH: no margin, no gap under the menu bar. The bar is lifted
 * over the menu bar while this look rests (`bar_stacking::OVER_MENU_BAR_LEVEL`
 * in Rust), so AppKit lets it sit at the very top edge.
 *
 * Pure, and in global desktop points like `snapWells.ts`.
 */

import { axisStops, type MonitorRect, type Well, type WellSlot } from "./snapWells";

/** Which of its two forms the Notch takes in a well. */
export type NotchForm = "notch" | "curl";

/** Which screen edge a notch grows out of. */
export type NotchEdge = "top" | "bottom" | "left" | "right";

/** Which screen corner a curl is peeled back from. */
export type CurlCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

/**
 * The resting notch's length along its edge, in points. Wider than the camera
 * housing on every notched MacBook (about 185 to 200 points) so the shape
 * covers it whole and leaves a small ear either side, where the status dot
 * shows. On a display without a notch it reads as a small black tab.
 */
export const NOTCH_REST_WIDTH = 236;

/** A resting notch on the left or right edge: shorter, since a side has no camera to cover. */
export const SIDE_REST_LENGTH = 72;

/** How far a resting notch on the bottom or a side stands out from its edge, in points. */
export const EDGE_REST_DEPTH = 24;

/**
 * The strip the shape draws over the menu bar when the display reports none
 * (the preview, a menu bar set to hide). A standard menu bar is 24 points; a
 * notched one is 37 or 38.
 */
export const FALLBACK_MENU_BAR = 32;

/** The resting curl: the corner peeled back this far along each edge, in points. */
export const CURL_REST = 44;

function isCorner(slot: WellSlot): boolean {
  return (slot.fx === 0 || slot.fx === 1) && (slot.fy === 0 || slot.fy === 1);
}

/** A corner is a curl; every other edge stop is a notch on that edge. */
export function notchFormFor(slot: WellSlot | null): NotchForm {
  if (!slot) return "notch";
  return isCorner(slot) ? "curl" : "notch";
}

/** The edge a notch well grows out of. Top for anything that is not on another edge. */
export function notchEdgeFor(slot: WellSlot | null): NotchEdge {
  if (!slot) return "top";
  if (slot.fy === 0) return "top";
  if (slot.fy === 1) return "bottom";
  if (slot.fx === 0) return "left";
  if (slot.fx === 1) return "right";
  return "top";
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
  if (notchFormFor(slot) === "curl") return { width: CURL_REST, height: CURL_REST };
  switch (notchEdgeFor(slot)) {
    case "top":
      return { width: NOTCH_REST_WIDTH, height: menuBarHeight(m) };
    case "bottom":
      return { width: NOTCH_REST_WIDTH, height: EDGE_REST_DEPTH };
    default:
      return { width: EDGE_REST_DEPTH, height: SIDE_REST_LENGTH };
  }
}

/**
 * The slots on a display's perimeter: the grid's edge stops (three along a
 * laptop's sides, five along a wide display's long one) and its corners.
 * Never the centre, and never an inner stop of a five-stop grid.
 */
export function perimeterSlots(m: MonitorRect): WellSlot[] {
  const slots: WellSlot[] = [];
  for (const fy of axisStops(m.size.height)) {
    for (const fx of axisStops(m.size.width)) {
      const onEdge = fx === 0 || fx === 1 || fy === 0 || fy === 1;
      if (onEdge) slots.push({ fx, fy });
    }
  }
  return slots;
}

/**
 * The Notch's wells on every display: every perimeter stop, each flush with
 * the display's own edges and sized for the resting shape it lands as.
 */
export function notchWells(monitors: MonitorRect[]): Well[] {
  const wells: Well[] = [];
  monitors.forEach((m, monitorIndex) => {
    for (const slot of perimeterSlots(m)) {
      const { width, height } = notchRestSize(slot, m);
      const x = Math.round(m.position.x + slot.fx * (m.size.width - width));
      const y = Math.round(m.position.y + slot.fy * (m.size.height - height));
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
