/**
 * Pure coordinate helpers for the snap-well drop indicator.
 *
 * There is one overlay window per display, not one spanning them all. With
 * "Displays have separate Spaces" (the macOS default) a window can only be
 * drawn on one display, so an overlay spanning the union of every monitor
 * showed on whichever display held most of it and the wells on the others
 * were simply missing. Each overlay covers exactly its own display and draws
 * only that display's wells.
 *
 * The overlay window is sized in points, so its CSS space is its display's
 * frame with the origin at the display's top-left. Wells are in global
 * points too (`src/lib/snapWells.ts`), so a hole is a well minus the
 * display's origin, nothing more.
 *
 * Kept free of Tauri imports so it can be unit tested without a running window.
 */

import type { MonitorRect, Well } from "./snapWells";
import { WINDOW_LABELS } from "./constants.generated";

/** The first overlay's label; display `i > 0` gets `${base}-${i}`. */
export const SNAP_WELLS_OVERLAY_BASE = WINDOW_LABELS.SNAP_WELLS_OVERLAY;

/**
 * Which display an overlay window covers, from its label: the base label is
 * display 0, `snap-wells-overlay-2` is display 2. -1 for any other label.
 */
export function overlayDisplayIndex(label: string): number {
  if (label === SNAP_WELLS_OVERLAY_BASE) return 0;
  const prefix = `${SNAP_WELLS_OVERLAY_BASE}-`;
  if (!label.startsWith(prefix)) return -1;
  const n = Number(label.slice(prefix.length));
  return Number.isInteger(n) && n > 0 ? n : -1;
}

export interface OverlayRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A well as a rectangle in its display's overlay, whose origin is the display's top-left. */
export function wellToOverlayRect(well: Well, display: MonitorRect): OverlayRect {
  return {
    x: well.x - display.position.x,
    y: well.y - display.position.y,
    w: well.width,
    h: well.height,
  };
}

/** The wells one overlay draws: those on its own display. */
export function wellsForDisplay(wells: Well[], displayIndex: number): Well[] {
  return wells.filter((w) => w.monitorIndex === displayIndex);
}

export interface LogicalBounds {
  /** Union top-left, in points. */
  originX: number;
  originY: number;
  /** Union size, in points. */
  width: number;
  height: number;
}

/**
 * Union bounding box of all monitors in points: each monitor's Tauri
 * geometry divided by its own `scaleFactor`. Used by the listening glow,
 * which still spans every display.
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
