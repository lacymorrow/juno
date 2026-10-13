import type { SteadySpec } from "@/lib/steadyFrame";
import {
  CURL_REST,
  EDGE_REST_DEPTH,
  NOTCH_REST_WIDTH,
  SIDE_REST_LENGTH,
  type CurlCorner,
  type NotchEdge,
  type NotchForm,
} from "@/lib/notchWells";
import { ISLAND_SIZES, type IslandSize, type Posture } from "../island/islandModel";

/**
 * The Notch's shapes, as pure numbers.
 *
 * The Notch says everything the Island says, with the Island's own posture
 * table (`islandModel.ts`): the same dot, the same words, the same card. What
 * changes is the body. In the top notch the island's content hangs below the
 * menu bar strip, out from under the camera; on another edge it grows straight
 * out of that edge; in a corner it sits on the black of a page peeled back
 * from the corner, with the folded flap at its inner corner.
 *
 * Every size here is the shape the person sees; the steady frame places it
 * against the well's docked edges.
 */

/** The concave shoulder where the notch meets the screen's top edge, in points. */
export const SHOULDER = 6;

/** How far the corner of an open curl is folded back, in points. */
export const FOLD = 22;

/** The resting notch's bottom corners. The hardware's are about this round. */
export const NOTCH_REST_RADIUS = 9;

/** Room under (or over) the shape for the skill suggestions, in points. */
export const SUGGESTION_ROOM = 200;

/**
 * The Notch's steady frame: room for the card plus the menu bar strip, the
 * fold and the suggestions, in any well. `rest` only stands in until a well
 * is chosen; every well carries its own resting size.
 */
export const NOTCH_STEADY_SPEC: SteadySpec = {
  rest: { width: NOTCH_REST_WIDTH, height: 32 },
  max: {
    width: ISLAND_SIZES.card.width + 2 * SHOULDER + 28,
    height: 40 + ISLAND_SIZES.card.maxHeight + FOLD + SUGGESTION_ROOM,
  },
  stage: { width: ISLAND_SIZES.card.width + 2 * SHOULDER, height: 40 + ISLAND_SIZES.line.height },
  wells: "notch",
};

/** One frame of the body. */
export interface NotchShape {
  form: NotchForm;
  /** The edge a notch grows out of. Top for a curl (unused there). */
  edge: NotchEdge;
  /** The whole footprint, shoulders and fold included. */
  width: number;
  height: number;
  /** Bottom corner radius of the notch, or the content radius of a curl. */
  radius: number;
  /** How far the curl's corner is peeled; equal to the box at rest. */
  fold: number;
  /** Where the island's content sits inside the footprint. */
  content: { top: number; left: number; width: number; height: number };
}

/**
 * The shape for a posture.
 *
 * `size` is what the Island would draw for the posture; `strip` is the menu
 * bar the shape covers before anything can show (37 or 38 on a notched
 * MacBook). At rest the notch is the strip alone and the curl a peeled corner;
 * everything else is the island's own size, hung below the strip.
 */
export function notchShape(
  form: NotchForm,
  posture: Posture,
  size: IslandSize,
  strip: number,
  corner: CurlCorner = "top-left",
  edge: NotchEdge = "top",
): NotchShape {
  const resting = posture === "capsule";
  if (form === "notch") return edgeNotch(edge, resting, size, strip);

  if (resting) {
    return {
      form,
      edge: "top",
      width: CURL_REST,
      height: CURL_REST,
      radius: 0,
      fold: CURL_REST,
      content: { top: 0, left: 0, width: CURL_REST, height: CURL_REST },
    };
  }

  // A top corner covers the menu bar first, as the notch does; a bottom one
  // starts at the screen's edge. The fold's strip is on the side away from the
  // screen edge, so the flap never lies over the words.
  const top = corner.startsWith("top");
  const pad = top ? strip : 0;
  const height = pad + size.height + FOLD;
  return {
    form,
    edge: "top",
    width: size.width,
    height,
    radius: size.radius,
    fold: FOLD,
    content: { top: top ? pad : FOLD, left: 0, width: size.width, height: size.height },
  };
}

/**
 * A notch on one edge. The shoulders sit on the screen's edge, at either end
 * of the shape; the far corners are round.
 *
 * Top: the menu bar strip comes first and the island hangs below it. Bottom:
 * the island stands on the screen's bottom edge. Left and right: the island
 * grows sideways out of the edge, with shoulders above and below it.
 */
function edgeNotch(edge: NotchEdge, resting: boolean, size: IslandSize, strip: number): NotchShape {
  const radius = resting ? NOTCH_REST_RADIUS : size.radius;
  if (edge === "top" || edge === "bottom") {
    const minBody = NOTCH_REST_WIDTH - 2 * SHOULDER;
    const body = resting ? minBody : Math.max(size.width, minBody);
    const lead = edge === "top" ? strip : 0;
    const depth = resting ? (edge === "top" ? strip : EDGE_REST_DEPTH) : lead + size.height;
    return {
      form: "notch",
      edge,
      width: body + 2 * SHOULDER,
      height: depth,
      radius,
      fold: 0,
      // At rest the content is the whole tab: the dot sits in it.
      content: resting
        ? { top: 0, left: SHOULDER, width: body, height: depth }
        : { top: lead, left: SHOULDER, width: body, height: size.height },
    };
  }
  // Open, the body is exactly the island's height: a side has no camera to
  // clear, so nothing makes it longer than what it holds.
  const body = resting ? SIDE_REST_LENGTH - 2 * SHOULDER : size.height;
  const depth = resting ? EDGE_REST_DEPTH : size.width;
  return {
    form: "notch",
    edge,
    width: depth,
    height: body + 2 * SHOULDER,
    radius,
    fold: 0,
    content: { top: SHOULDER, left: 0, width: depth, height: body },
  };
}

/**
 * Where the resting dot sits inside the resting shape.
 *
 * In the top notch it is in the right-hand ear, clear of the camera; on any
 * other edge in the middle of the tab; in a curl it is in the black triangle,
 * close to the screen's corner.
 */
export function restingDotPlacement(
  form: NotchForm,
  corner: CurlCorner,
  edge: NotchEdge = "top",
): { justify: "start" | "end" | "center"; align: "start" | "end" | "center"; inset: number } {
  // Under the camera the dot moves to the ear; any other edge has no camera.
  if (form === "notch") {
    return edge === "top"
      ? { justify: "end", align: "center", inset: 8 }
      : { justify: "center", align: "center", inset: 0 };
  }
  return {
    justify: corner.endsWith("left") ? "start" : "end",
    align: corner.startsWith("top") ? "start" : "end",
    inset: 9,
  };
}

/**
 * The curl's black region as a CSS polygon, drawn for the top-left corner
 * (mirrored for the others): the box with its inner corner folded away along
 * a line `fold` long on each edge. At rest the fold is the whole box and the
 * region is a triangle: the corner of the screen, peeled back.
 */
export function curlUnderClip(width: number, height: number, fold: number): string {
  const f = Math.max(0, Math.min(fold, width, height));
  return `polygon(0px 0px, ${width}px 0px, ${width}px ${height - f}px, ${width - f}px ${height}px, 0px ${height}px)`;
}
