/**
 * The well the bar is docked in, and everything that follows from it.
 *
 * The bar always sits in a snap well (`src/lib/snapWells.ts`): placed there at
 * launch, on release after a drag, on a display hop. The well is not just a
 * position, it decides how the window grows: away from the docked screen
 * edge, never towards it. A bar in a right-hand well grows leftward; one in
 * the bottom half opens its pane upward.
 *
 * That slot used to live as React state inside FloatingBar, which is why only
 * the Pill grew correctly: every other look resized centre-anchored and was
 * then nudged back inside the screen by `clampToMonitor`, so an Island parked
 * at a right-edge well drifted off it and never returned. The slot lives here
 * instead, keyed by window label, so `useWindowSize` can default `anchorX` and
 * `growUp` from it for whichever look happens to be mounted.
 *
 * Everything here is pure except the slot store, which is a plain
 * subscribe/notify box with no Tauri imports, so all of it is unit testable
 * without a running window.
 */

import type { MonitorRect, Well, WellSlot } from "./snapWells";

/** Which horizontal edge of the window stays put across a resize. */
export type WindowAnchorX = "start" | "center" | "end";

/** Where the docked well puts the window's fixed horizontal edge. */
export function dockAnchorX(dock: WellSlot | null): WindowAnchorX {
  if (!dock) return "center";
  return dock.fx === 0 ? "start" : dock.fx === 1 ? "end" : "center";
}

/** A well in the bottom half of its display opens the pane upward. */
export function dockGrowsUp(dock: WellSlot | null): boolean {
  return dock !== null && dock.fy >= 0.5;
}

/**
 * One well per real landing spot.
 *
 * `computeWells` insets every well by the window's own footprint, so a look
 * wider than the inset area has its columns converge: a 5000px-wide window on
 * a 1000px display yields three columns at the same x. They are the same place
 * on screen, and the drop indicator must draw one ring there, not three
 * stacked on top of each other (which read as a brighter ring, and made the
 * highlight look like it was lighting two wells at once).
 *
 * Coincidence is exact equality of monitor and top-left, which is what
 * `computeWells` produces when an axis collapses; nothing is rounded here.
 * The first well at each spot wins, so the order stays the one `computeWells`
 * emitted.
 */
export function distinctWells(wells: Well[]): Well[] {
  const seen = new Set<string>();
  const out: Well[] = [];
  for (const w of wells) {
    const key = `${w.monitorIndex}:${w.x}:${w.y}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(w);
  }
  return out;
}

/**
 * Which monitor a physical point is on, or -1 when it is on none of them (the
 * gap between two displays of different heights is a real place a window's
 * corner can sit). Callers decide what "none" means for them.
 */
export function monitorIndexAt(
  mons: Array<{ position: { x: number; y: number }; size: { width: number; height: number } }>,
  x: number,
  y: number,
): number {
  return mons.findIndex(
    (m) =>
      x >= m.position.x &&
      x < m.position.x + m.size.width &&
      y >= m.position.y &&
      y < m.position.y + m.size.height,
  );
}

/**
 * Where the dragged window's top-left is, worked out from the cursor.
 *
 * During an OS window drag the window keeps the grab point under the cursor,
 * so its top-left is the cursor minus the offset the press landed at inside
 * the window. The cursor arrives in physical pixels and the grab offset is in
 * logical ones (it came from a DOM `clientX`/`clientY`), so the offset is
 * scaled by the factor of whichever display the cursor is on.
 *
 * This is what makes the drop indicator agree with where the bar lands: the
 * overlay used to compare the bare cursor against each well's centre, while
 * the settle compares the window's top-left against each well's top-left.
 * Those agree only while the window is small. On a wide strip grabbed near one
 * end the error is half the window width, easily a whole column, so the ring
 * brightened over one well and the bar landed in another.
 */
export function predictedWindowOrigin(
  cursor: { x: number; y: number },
  grabOffset: { x: number; y: number },
  monitors: MonitorRect[],
): { x: number; y: number } {
  const idx = monitorIndexAt(monitors, cursor.x, cursor.y);
  const sf = (idx >= 0 ? monitors[idx]?.scaleFactor : monitors[0]?.scaleFactor) || 1;
  return {
    x: cursor.x - grabOffset.x * sf,
    y: cursor.y - grabOffset.y * sf,
  };
}

// ── The dock slot store ───────────────────────────────────────────────────
//
// One slot per window label. A plain box rather than React state because two
// unrelated readers need it: whichever bar look is mounted (to lay itself out
// against the docked edges) and `useWindowSize` (to anchor every resize on
// them), and the second is not a component.

const slotByLabel = new Map<string, WellSlot | null>();
const listenersByLabel = new Map<string, Set<() => void>>();

/** The well the window with this label is docked in, or null before it lands. */
export function getDockSlot(label: string): WellSlot | null {
  return slotByLabel.get(label) ?? null;
}

/**
 * Record where the window landed. A no-op when the slot is unchanged, so a
 * re-settle into the same well cannot wake every subscriber.
 */
export function setDockSlot(label: string, slot: WellSlot | null): void {
  const prev = slotByLabel.get(label) ?? null;
  if (prev === slot) return;
  if (prev && slot && prev.fx === slot.fx && prev.fy === slot.fy) return;
  slotByLabel.set(label, slot);
  const listeners = listenersByLabel.get(label);
  if (listeners) for (const fn of [...listeners]) fn();
}

export function subscribeDockSlot(label: string, fn: () => void): () => void {
  let set = listenersByLabel.get(label);
  if (!set) {
    set = new Set();
    listenersByLabel.set(label, set);
  }
  set.add(fn);
  return () => {
    set?.delete(fn);
  };
}

/** Forget every slot. For tests, which share one module instance per file. */
export function resetDockSlots(): void {
  slotByLabel.clear();
}
