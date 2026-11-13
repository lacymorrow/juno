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

import type { Well, WellSlot } from "./snapWells";

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
 * Which monitor a point is on, or -1 when it is on none of them (the gap
 * between two displays of different heights is a real place a window's
 * corner can sit). Callers decide what "none" means for them. Point and
 * monitors must be in the same space: global points everywhere in the bar.
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
 * Where the dragged footprint's top-left is, worked out from the cursor.
 *
 * During a drag the bar keeps the grab point under the cursor, so the
 * footprint's top-left is the cursor minus the offset the press landed at
 * inside it. Both are in points (the offset came from a DOM `clientX`), so
 * this holds on every display whatever its pixel density.
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
): { x: number; y: number } {
  return { x: cursor.x - grabOffset.x, y: cursor.y - grabOffset.y };
}

// ── The dock slot store ───────────────────────────────────────────────────
//
// One slot per window label. A plain box rather than React state because two
// unrelated readers need it: whichever bar look is mounted (to lay itself out
// against the docked edges) and `useWindowSize` (to anchor every resize on
// them), and the second is not a component.
//
// Two slots per label, not one, and the difference is the whole of LAC-4831.
// The **dock slot** is where the bar is drawn right now, which is what every
// layout reader wants. The **intent** is the well someone put it in, which is
// what a display hop has to project from. They agree until the bar visits a
// display that does not have that well: `axisStops` gives an axis of
// `FIVE_STOP_MIN_LOGICAL` or more five stops and a shorter one three, so a bar
// dragged to the quarter-point column on a 5K has no such column to sit in on
// a laptop and is *drawn* in the centre one there.
//
// With one slot, that projection was written back over the choice: the hop
// destroyed it, and the bar came home to the 5K's centre column instead of the
// quarter-point one it was dragged to. Projection is lossy by nature and must
// stay that way, because the bar has to be drawn somewhere. What must not be
// lossy is the record of what was asked for. So the hop projects and leaves
// the intent alone, and the round trip comes back exact.

const slotByLabel = new Map<string, WellSlot | null>();
const intentByLabel = new Map<string, WellSlot | null>();
const listenersByLabel = new Map<string, Set<() => void>>();

/** The well the window with this label is docked in, or null before it lands. */
export function getDockSlot(label: string): WellSlot | null {
  return slotByLabel.get(label) ?? null;
}

/**
 * The well someone last put this window in: a drag release, the tray's
 * "Center Floating Bar", the position restored at launch. A display hop never
 * writes it, so it is still the quarter-point column after a trip across a
 * three-stop laptop. Falls back to the drawn slot, which is all there is
 * before anything has been chosen.
 */
export function getDockIntent(label: string): WellSlot | null {
  return intentByLabel.get(label) ?? slotByLabel.get(label) ?? null;
}

/**
 * Record where the window landed because someone put it there. Sets the drawn
 * slot and the intent together: every caller but the display hop is a choice,
 * so this stays the default and the exception has to say so.
 */
export function setDockSlot(label: string, slot: WellSlot | null): void {
  intentByLabel.set(label, slot);
  writeDockSlot(label, slot);
}

/**
 * Record where the window is drawn after a move nobody asked for: the display
 * hop, which puts the bar on the cursor's display in the nearest well that
 * display happens to have. The intent is left untouched, so the bar returns to
 * the well it was dragged to the moment it is back on a display with one.
 */
export function setDockProjection(label: string, slot: WellSlot | null): void {
  writeDockSlot(label, slot);
}

/**
 * Move the drawn slot and wake its readers. A no-op when the slot is
 * unchanged, so a re-settle into the same well cannot wake every subscriber.
 */
function writeDockSlot(label: string, slot: WellSlot | null): void {
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
  intentByLabel.clear();
}
