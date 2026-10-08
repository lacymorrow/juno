/**
 * Tauri's screen numbers, turned into global desktop points.
 *
 * macOS lays every display out in one coordinate space measured in points
 * (NSScreen frames, CSS pixels, `LogicalPosition`): top-left origin at the
 * primary display, y down after the flip. It is the only space in which two
 * displays of different pixel density sit side by side without gaps or
 * overlaps, so it is the only space the bar's geometry works in.
 *
 * Tauri (tao 0.35 on macOS) reports three different "physical" spaces:
 *
 * - a monitor's `position` and `size` are its points times ITS OWN scale
 *   factor (`CGDisplayBounds` x `backingScaleFactor`);
 * - `cursorPosition()` is the cursor's points times the PRIMARY display's
 *   scale factor (`NSEvent.mouseLocation` x the main display's factor);
 * - a window's `outerPosition()` is its points times the scale factor of the
 *   display the window is on, and `setPosition(PhysicalPosition)` divides by
 *   that same factor.
 *
 * On one display, or on displays of equal density, the three agree. With a
 * 1x external next to a 2x Retina they do not: the external monitor's
 * physical rect overlaps the laptop's, the cursor on the external lands in
 * neither, and a well computed for one display is placed on the other. Each
 * conversion below undoes exactly the factor tao applied, which makes it
 * exact (up to tao's integer rounding).
 *
 * Pure, so it is unit tested without a window.
 */

import type { MonitorRect } from "./snapWells";

/** The shape of a Tauri `Monitor`, as far as geometry is concerned. */
export interface TauriMonitorLike {
  position: { x: number; y: number };
  size: { width: number; height: number };
  scaleFactor?: number;
  /**
   * The part of the display not taken by the menu bar or the Dock
   * (`NSScreen.visibleFrame`), scaled the same way as `position`/`size`.
   */
  workArea?: {
    position: { x: number; y: number };
    size: { width: number; height: number };
  };
}

function factor(sf: number | undefined): number {
  return sf && sf > 0 ? sf : 1;
}

/** Every monitor's frame in global points, in Tauri's order. */
export function monitorsInPoints(mons: TauriMonitorLike[]): MonitorRect[] {
  return mons.map((m) => {
    const sf = factor(m.scaleFactor);
    const rect: MonitorRect = {
      position: { x: Math.round(m.position.x / sf), y: Math.round(m.position.y / sf) },
      size: { width: Math.round(m.size.width / sf), height: Math.round(m.size.height / sf) },
      scaleFactor: sf,
    };
    // tao builds the work area from the monitor's own point origin plus the
    // visible frame's insets, then multiplies by the monitor's own factor, so
    // dividing by that factor gives it back in global points.
    const wa = m.workArea;
    if (wa && wa.size.width > 0 && wa.size.height > 0) {
      rect.workArea = {
        position: { x: Math.round(wa.position.x / sf), y: Math.round(wa.position.y / sf) },
        size: { width: Math.round(wa.size.width / sf), height: Math.round(wa.size.height / sf) },
      };
    }
    return rect;
  });
}

/**
 * The primary display's scale factor: the display at the origin (the one with
 * the menu bar in System Settings > Displays), which is what tao multiplies
 * the cursor by. Falls back to the first monitor, then to 1.
 */
export function primaryScale(mons: TauriMonitorLike[]): number {
  const primary = mons.find((m) => m.position.x === 0 && m.position.y === 0) ?? mons[0];
  return factor(primary?.scaleFactor);
}

/** Tauri's cursor position in global points. */
export function cursorInPoints(
  cursor: { x: number; y: number },
  mons: TauriMonitorLike[],
): { x: number; y: number } {
  const sf = primaryScale(mons);
  return { x: cursor.x / sf, y: cursor.y / sf };
}

/** A window's `outerPosition()` in global points, given its `scaleFactor()`. */
export function windowOriginInPoints(
  pos: { x: number; y: number },
  windowScale: number,
): { x: number; y: number } {
  const sf = factor(windowScale);
  return { x: Math.round(pos.x / sf), y: Math.round(pos.y / sf) };
}
