/**
 * Pure coordinate helpers for the snap-well drop indicator overlay.
 *
 * The overlay window spans the union of every monitor (see SnapWellsOverlay).
 * Like DesktopCursorOverlay, its size is set in **logical** pixels, so its DOM
 * / CSS coordinate space has its origin at the union box's logical top-left.
 *
 * Snap wells (`src/lib/snapWells.ts`) are computed in **physical** pixels. These
 * helpers turn a well (plus the bar's physical outer size) into a rectangle in
 * the overlay's local CSS space so the cut-out holes line up with where the bar
 * will actually land, across displays with different scale factors.
 *
 * Kept free of Tauri imports so it can be unit tested without a running window.
 */

import type { MonitorRect } from "./snapWells";

export interface LogicalBounds {
  /** Union top-left, in logical pixels. */
  originX: number;
  originY: number;
  /** Union size, in logical pixels. */
  width: number;
  height: number;
}

/**
 * Union bounding box of all monitors in LOGICAL pixels — each monitor's
 * physical geometry divided by its own `scaleFactor`. Mirrors the span
 * computation in DesktopCursorOverlay so the overlay covers the same area.
 */
export function unionLogicalBounds(monitors: MonitorRect[]): LogicalBounds {
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const m of monitors) {
    const sf = m.scaleFactor || 1;
    const lx = m.position.x / sf;
    const ly = m.position.y / sf;
    const lw = m.size.width / sf;
    const lh = m.size.height / sf;
    if (lx < minX) minX = lx;
    if (ly < minY) minY = ly;
    if (lx + lw > maxX) maxX = lx + lw;
    if (ly + lh > maxY) maxY = ly + lh;
  }

  if (!Number.isFinite(minX) || !Number.isFinite(maxX)) {
    return { originX: 0, originY: 0, width: 0, height: 0 };
  }
  return { originX: minX, originY: minY, width: maxX - minX, height: maxY - minY };
}

export interface OverlayRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Convert a well (physical px, tagged with its monitor) plus the bar's physical
 * outer size into a rectangle in the overlay's local CSS space, whose origin is
 * the union box's logical top-left. Divides by the well's monitor scale factor
 * to land in the same logical space the overlay window is sized in.
 */
export function wellToOverlayRect(
  well: { x: number; y: number; monitorIndex: number },
  monitors: MonitorRect[],
  origin: { x: number; y: number },
  barSize: { width: number; height: number },
): OverlayRect {
  const sf = monitors[well.monitorIndex]?.scaleFactor || 1;
  return {
    x: well.x / sf - origin.x,
    y: well.y / sf - origin.y,
    w: barSize.width / sf,
    h: barSize.height / sf,
  };
}
