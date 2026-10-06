/**
 * The steady frame: one window per dock well, never resized per state.
 *
 * Why it exists. A look that resizes its window on every state change (idle,
 * hover, listening, answer, pane) asks two processes to agree on one frame:
 * AppKit moves and resizes the NSWindow, and WebKit lays the page out again at
 * the new viewport. They land on different display frames. For a frame or two
 * the old page is drawn in the new window, pinned to its top-left, so anything
 * anchored to the right or bottom edge visibly jumps and snaps back. Which way
 * it jumps depends on which edge moved, which is why the jump changed direction
 * with the well the bar was docked in. No anchoring maths can fix that; the
 * seam is between the processes.
 *
 * The pattern notch and island apps use (NotchNook, Boring Notch, Alcove, the
 * Wispr Flow pill): the window is sized once for the LARGEST state the look can
 * reach, placed once per well, and the visible shape grows and shrinks in CSS
 * inside it, pinned to the docked edge. The transparent rest of the window lets
 * clicks through (Rust hit-tests the cursor against the content rect the page
 * reports, see `src-tauri/src/platform/bar_hit_test.rs`). A state change is
 * then a CSS transition and nothing else: the window never moves, so the docked
 * edge cannot.
 *
 * Geometry, in words. Wells are still computed for the look's RESTING
 * footprint (`spec.rest`), so the resting shape sits exactly where it always
 * did. That footprint inside the window is the **anchor rect**. The window
 * extends away from the docked edges: rightward from a left well, leftward
 * from a right well, both ways from a centre column, downward from a top-half
 * well and upward from a bottom-half one. The window is then clamped inside
 * the display's inset area, and the clamp is absorbed by the anchor offset,
 * never by the anchor itself: the anchor rect is always the well.
 *
 * Everything here is pure except the small per-label store at the bottom.
 * Positions are physical pixels (Tauri's units), sizes logical, as in
 * `snapWells.ts`.
 */

import type { MonitorRect, Well, WellSlot } from "./snapWells";
import { dockAnchorX, dockGrowsUp, type WindowAnchorX } from "./barDock";

/** A size in logical pixels. */
export interface Size {
  width: number;
  height: number;
}

/** A rectangle in logical pixels, relative to the window's top-left. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What a look tells the steady frame about itself. All sizes logical. */
export interface SteadySpec {
  /** The resting footprint wells are computed for (shape plus its padding). */
  rest: Size;
  /** The largest footprint any state can reach. The window is this size. */
  max: Size;
  /**
   * The region the appearance preview frames: the shape's own extent at its
   * widest, without panes. Anchored at the same docked edges as `rest`.
   */
  stage: Size;
}

/** Where a look's window sits for one well, and where its content goes inside it. */
export interface SteadyLayout {
  slot: WellSlot;
  monitorIndex: number;
  /** The window's top-left, physical pixels. */
  origin: { x: number; y: number };
  /** The window's size, logical pixels: `spec.max`, never changed per state. */
  size: Size;
  /** The resting footprint inside the window. On screen it is exactly the well. */
  anchor: Rect;
  /** The docked column: which horizontal edge of the anchor content is pinned to. */
  anchorX: WindowAnchorX;
  /** Content grows up from the anchor's bottom edge instead of down from its top. */
  growUp: boolean;
  /** Scale factor of the display this layout was computed for. */
  scaleFactor: number;
}

/** Inset of the area a steady window is kept inside. Matches `computeWells`. */
export interface SteadyInsets {
  margin?: number;
  topInset?: number;
}

/**
 * The steady layout for one well.
 *
 * `well` is a well computed for `spec.rest` (its x/y is the resting
 * footprint's top-left). The anchor offset is how far that footprint sits from
 * the window's top-left, in logical pixels; it is an integer so every edge
 * lands on a pixel.
 */
export function steadyLayout(
  well: Well,
  monitor: MonitorRect,
  spec: SteadySpec,
  { margin = 16, topInset = 36 }: SteadyInsets = {},
): SteadyLayout {
  const sf = monitor.scaleFactor || 1;
  const slot = { fx: well.fx, fy: well.fy };
  const anchorX = dockAnchorX(slot);
  const growUp = dockGrowsUp(slot);
  const rw = spec.rest.width;
  const rh = spec.rest.height;
  // One pixel wider when the room either side of a centred footprint would
  // otherwise be uneven: a centre column then sits on a whole pixel, and so
  // does every edge that grows symmetrically around it.
  const W = spec.max.width + ((spec.max.width - rw) % 2);
  const H = spec.max.height;

  // Ideal offset of the resting footprint inside the window, logical.
  const idealX = anchorX === "start" ? 0 : anchorX === "end" ? W - rw : Math.floor((W - rw) / 2);
  const idealY = growUp ? H - rh : 0;

  // The window's ideal top-left (physical), then clamped inside the inset area.
  const minX = monitor.position.x + Math.round(margin * sf);
  const maxX = monitor.position.x + monitor.size.width - Math.round(margin * sf) - Math.round(W * sf);
  const minY = monitor.position.y + Math.round(topInset * sf);
  const maxY = monitor.position.y + monitor.size.height - Math.round(margin * sf) - Math.round(H * sf);
  const clamp = (v: number, lo: number, hi: number) => (hi < lo ? lo : Math.min(hi, Math.max(lo, v)));

  // Clamping works in whole logical pixels so the anchor offset stays an
  // integer: the anchor is where the well is, and the window moves around it.
  const ox = clamp(well.x - Math.round(idealX * sf), minX, maxX);
  const oy = clamp(well.y - Math.round(idealY * sf), minY, maxY);
  const ax = Math.round((well.x - ox) / sf);
  const ay = Math.round((well.y - oy) / sf);
  const origin = { x: well.x - Math.round(ax * sf), y: well.y - Math.round(ay * sf) };

  return {
    slot,
    monitorIndex: well.monitorIndex,
    origin,
    size: { width: W, height: H },
    anchor: { x: ax, y: ay, width: rw, height: rh },
    anchorX,
    growUp,
    scaleFactor: sf,
  };
}

/**
 * Where a block of content of `size` sits inside the window: pinned to the
 * anchor's docked edges, growing away from them. This is the whole rule the
 * render follows, so it is what the tests pin.
 */
export function contentRect(layout: SteadyLayout, size: Size): Rect {
  const { anchor } = layout;
  const x =
    layout.anchorX === "start"
      ? anchor.x
      : layout.anchorX === "end"
        ? anchor.x + anchor.width - size.width
        : anchor.x + anchor.width / 2 - size.width / 2;
  const y = layout.growUp ? anchor.y + anchor.height - size.height : anchor.y;
  return { x, y, width: size.width, height: size.height };
}

/**
 * CSS placement for the content block: the docked edges as absolute offsets,
 * so the browser holds those edges still while width and height animate.
 * A centred column is pinned at its centre line with a -50% translate.
 */
export function contentPlacement(layout: SteadyLayout): {
  left?: number;
  right?: number;
  top?: number;
  bottom?: number;
  translateX: boolean;
} {
  const { anchor, size } = layout;
  const horizontal =
    layout.anchorX === "start"
      ? { left: anchor.x, translateX: false }
      : layout.anchorX === "end"
        ? { right: size.width - (anchor.x + anchor.width), translateX: false }
        : { left: anchor.x + anchor.width / 2, translateX: true };
  const vertical = layout.growUp
    ? { bottom: size.height - (anchor.y + anchor.height) }
    : { top: anchor.y };
  return { ...horizontal, ...vertical };
}

/**
 * Room for the content block along its growth axis: from the anchor's docked
 * edge to the far edge of the window. A look caps its tallest part (the Pill's
 * pane) to this, so a window clamped by a small display never clips.
 */
export function roomForContent(layout: SteadyLayout): number {
  return layout.growUp
    ? layout.anchor.y + layout.anchor.height
    : layout.size.height - layout.anchor.y;
}

/** A logical rect in the window, as a physical rect on screen. */
export function toScreen(layout: SteadyLayout, r: Rect): Rect {
  const sf = layout.scaleFactor;
  return {
    x: layout.origin.x + r.x * sf,
    y: layout.origin.y + r.y * sf,
    width: r.width * sf,
    height: r.height * sf,
  };
}

/**
 * The screen coordinate of the docked edges of a content block: x of the
 * pinned column edge (left, right or centre line) and y of the pinned row edge
 * (top, or bottom when growing up). Identical for every state is the contract.
 */
export function dockedEdges(layout: SteadyLayout, size: Size): { x: number; y: number } {
  const r = toScreen(layout, contentRect(layout, size));
  const x =
    layout.anchorX === "start" ? r.x : layout.anchorX === "end" ? r.x + r.width : r.x + r.width / 2;
  const y = layout.growUp ? r.y + r.height : r.y;
  return { x, y };
}

/** The resting footprint's top-left on screen: the well this layout belongs to. */
export function anchorScreenOrigin(layout: SteadyLayout): { x: number; y: number } {
  const sf = layout.scaleFactor;
  return {
    x: layout.origin.x + Math.round(layout.anchor.x * sf),
    y: layout.origin.y + Math.round(layout.anchor.y * sf),
  };
}

/** Whether moving between two layouts changes anything the page draws. */
export function sameLayout(a: SteadyLayout | null, b: SteadyLayout | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.origin.x === b.origin.x &&
    a.origin.y === b.origin.y &&
    a.size.width === b.size.width &&
    a.size.height === b.size.height &&
    a.anchor.x === b.anchor.x &&
    a.anchor.y === b.anchor.y &&
    a.anchorX === b.anchorX &&
    a.growUp === b.growUp &&
    a.scaleFactor === b.scaleFactor
  );
}

/** Whether only the window's position differs: the page draws the same thing. */
export function sameDrawing(a: SteadyLayout, b: SteadyLayout): boolean {
  return (
    a.size.width === b.size.width &&
    a.size.height === b.size.height &&
    a.anchor.x === b.anchor.x &&
    a.anchor.y === b.anchor.y &&
    a.anchorX === b.anchorX &&
    a.growUp === b.growUp &&
    a.scaleFactor === b.scaleFactor
  );
}

// ── The per-label store ───────────────────────────────────────────────────
//
// A look that uses the steady frame registers its spec here, and whoever
// places the window (launch placement, the settle after a drag, the display
// hop) records the layout. The look draws from the recorded layout, so the
// window frame and the drawing change in the same step. `hidden` is set while
// a layout swap is in flight (see `swapSteadyLayout`).

interface SteadyEntry {
  spec: SteadySpec;
  layout: SteadyLayout | null;
  hidden: boolean;
}

const entries = new Map<string, SteadyEntry>();
const listeners = new Map<string, Set<() => void>>();
const epochs = new Map<string, number>();

/**
 * Counts the frames a steady look has set on a window. `useWindowSize` keeps
 * this beside its last-applied size, so a look that resizes through it after a
 * steady look has had the window never skips a resize as a no-op.
 */
export function frameEpoch(label: string): number {
  return epochs.get(label) ?? 0;
}

export function bumpFrameEpoch(label: string): void {
  epochs.set(label, frameEpoch(label) + 1);
}

function notify(label: string): void {
  const set = listeners.get(label);
  if (set) for (const fn of [...set]) fn();
}

/** A look opts in. Null opts out (the look unmounted). */
export function registerSteady(label: string, spec: SteadySpec | null): void {
  if (spec) entries.set(label, { spec, layout: entries.get(label)?.layout ?? null, hidden: false });
  else entries.delete(label);
  notify(label);
}

export function getSteady(label: string): SteadyEntry | null {
  return entries.get(label) ?? null;
}

export function setSteadyLayout(label: string, layout: SteadyLayout): void {
  bumpFrameEpoch(label);
  const e = entries.get(label);
  if (!e) return;
  entries.set(label, { ...e, layout });
  notify(label);
}

export function setSteadyHidden(label: string, hidden: boolean): void {
  const e = entries.get(label);
  if (!e || e.hidden === hidden) return;
  entries.set(label, { ...e, hidden });
  notify(label);
}

export function subscribeSteady(label: string, fn: () => void): () => void {
  let set = listeners.get(label);
  if (!set) {
    set = new Set();
    listeners.set(label, set);
  }
  set.add(fn);
  return () => {
    set?.delete(fn);
  };
}

/** Forget everything. For tests. */
export function resetSteady(): void {
  entries.clear();
}

/** Two animation frames: long enough for a committed style to reach the screen. */
export function afterPaint(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame !== "function") {
      setTimeout(resolve, 32);
      return;
    }
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

/**
 * Move a steady window to a new layout.
 *
 * When only the position changes (same well side, another display spot) the
 * frame moves and the drawing is untouched: nothing can jump. When the drawing
 * changes too (the bar landed on the other side or half of the display, so it
 * now grows the other way) the frame and the page cannot be changed in the
 * same display frame, which is the very seam the steady frame exists to avoid.
 * That swap is done hidden: the content is made invisible, the frame and the
 * drawing change, and it comes back once both have reached the screen. It
 * happens only on a drop or a display hop, never on a state change.
 */
export async function swapSteadyLayout(
  label: string,
  next: SteadyLayout,
  applyFrame: (layout: SteadyLayout) => Promise<void>,
): Promise<void> {
  const prev = getSteady(label)?.layout ?? null;
  if (prev && sameDrawing(prev, next)) {
    await applyFrame(next);
    setSteadyLayout(label, next);
    return;
  }
  setSteadyHidden(label, true);
  try {
    await afterPaint();
    await applyFrame(next);
    setSteadyLayout(label, next);
    await afterPaint();
  } finally {
    setSteadyHidden(label, false);
  }
}
