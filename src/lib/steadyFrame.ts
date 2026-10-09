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
 * Everything is in global desktop points, as in `snapWells.ts`.
 */

import { screenInsets, type MonitorRect, type Well, type WellSlot } from "./snapWells";
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
  /** The window's top-left, global points. */
  origin: { x: number; y: number };
  /** The window's size, points: `spec.max`, never changed per state. */
  size: Size;
  /** The resting footprint inside the window. On screen it is exactly the well. */
  anchor: Rect;
  /** The docked column: which horizontal edge of the anchor content is pinned to. */
  anchorX: WindowAnchorX;
  /** Content grows up from the anchor's bottom edge instead of down from its top. */
  growUp: boolean;
}

/**
 * Inset of the area a steady window is kept inside, when the display reports
 * no work area. Matches `computeWells`; both go through `screenInsets`.
 */
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
  insets: SteadyInsets = {},
): SteadyLayout {
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

  // Ideal offset of the resting footprint inside the window.
  const idealX = anchorX === "start" ? 0 : anchorX === "end" ? W - rw : Math.floor((W - rw) / 2);
  const idealY = growUp ? H - rh : 0;

  // The window's ideal top-left, then clamped inside the inset area: the
  // display's real work area, so the top edge sits under this display's own
  // menu bar and the window stays clear of the Dock.
  const inset = screenInsets(monitor, insets);
  const minX = monitor.position.x + inset.left;
  const maxX = monitor.position.x + monitor.size.width - inset.right - W;
  const minY = monitor.position.y + inset.top;
  const maxY = monitor.position.y + monitor.size.height - inset.bottom - H;
  const clamp = (v: number, lo: number, hi: number) => (hi < lo ? lo : Math.min(hi, Math.max(lo, v)));

  // The clamp is absorbed by the anchor offset, never by the anchor: the
  // anchor is where the well is, and the window moves around it.
  const ox = clamp(well.x - idealX, minX, maxX);
  const oy = clamp(well.y - idealY, minY, maxY);
  const ax = well.x - ox;
  const ay = well.y - oy;

  return {
    slot,
    monitorIndex: well.monitorIndex,
    origin: { x: ox, y: oy },
    size: { width: W, height: H },
    anchor: { x: ax, y: ay, width: rw, height: rh },
    anchorX,
    growUp,
  };
}

/**
 * The layout a steady look is dragged in: the window becomes exactly what is
 * drawn. Drag the shape, not the stage.
 *
 * Why. macOS keeps a floating window's top edge below the menu bar on every
 * frame change, the OS drag included (AppKit 10.9 extended
 * `constrainFrameRect:toScreen:` to borderless windows below
 * `NSMainMenuWindowLevel`). The steady window is far taller than the pill, so
 * a pill docked low stopped halfway up the screen. A window that is only the
 * footprint is held exactly where the pill should be held: its top just
 * under the menu bar, which is where the top wells are. It also changes
 * display with the shape, because it is the shape.
 *
 * `footprint` is what the look is drawing at drag start. Everything that
 * says how the look draws (the docked column, the growth direction) is kept,
 * so nothing inside the shape moves; the anchor (the resting footprint) keeps
 * its place on screen, so the wells, the grab and the settle all still refer
 * to it; and the window is placed so the footprint is at its top-left.
 */
export function dragLayout(layout: SteadyLayout, footprint: Size): SteadyLayout {
  const drawn = contentRect(layout, footprint);
  return {
    ...layout,
    origin: { x: layout.origin.x + drawn.x, y: layout.origin.y + drawn.y },
    size: { width: footprint.width, height: footprint.height },
    anchor: { ...layout.anchor, x: layout.anchor.x - drawn.x, y: layout.anchor.y - drawn.y },
  };
}

/**
 * Where the press sits inside the drag window (`dragLayout(layout,
 * footprint)`), given where it landed inside the steady window. The drag
 * window starts at the footprint's top-left, so it is the same spot on the
 * shape, re-measured from there. The drag window is placed so this point is
 * under the cursor (`originUnderCursor`), which keeps the spot the user
 * grabbed under the cursor for the whole drag, whatever the well (which edge
 * the footprint is pinned to) and whatever the state (how big it is).
 */
export function grabInDragWindow(
  layout: SteadyLayout,
  footprint: Size,
  grab: { x: number; y: number },
): { x: number; y: number } {
  const drawn = contentRect(layout, footprint);
  return { x: grab.x - drawn.x, y: grab.y - drawn.y };
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

/** A rect in the window, as a rect on screen (both in points). */
export function toScreen(layout: SteadyLayout, r: Rect): Rect {
  return {
    x: layout.origin.x + r.x,
    y: layout.origin.y + r.y,
    width: r.width,
    height: r.height,
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
  return {
    x: layout.origin.x + layout.anchor.x,
    y: layout.origin.y + layout.anchor.y,
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
    a.growUp === b.growUp
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
    a.growUp === b.growUp
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
  /**
   * What the look is drawing right now, in points. The drag shrinks the
   * window to it. Not a render input, so setting it notifies nobody.
   */
  footprint: Size | null;
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
  if (spec) {
    const prev = entries.get(label);
    entries.set(label, {
      spec,
      layout: prev?.layout ?? null,
      hidden: false,
      footprint: prev?.footprint ?? null,
    });
  }
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

/** Record what the look is drawing. Read at drag start; renders nothing. */
export function setSteadyFootprint(label: string, footprint: Size): void {
  const e = entries.get(label);
  if (!e) return;
  e.footprint = { width: footprint.width, height: footprint.height };
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
  swapTails.clear();
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

// One swap at a time per window. Two used to interleave (a display hop in
// flight when a drag started): `hidden` is one flag, so the first swap's
// `finally` showed the shape while the second was still mid-flight, and for a
// frame the wrong layout was on screen. Each swap now waits for the one
// before it.
const swapTails = new Map<string, Promise<void>>();

/**
 * Puts a layout's frame on the window. It may return the layout as actually
 * placed (the drag window is placed by the cursor, not by `layout.origin`);
 * that is what the store then holds.
 */
export type ApplyFrame = (layout: SteadyLayout) => Promise<SteadyLayout | void>;

/** Resolves once every swap already asked for on `label` has finished. */
export function steadySwapsSettled(label: string): Promise<void> {
  return swapTails.get(label) ?? Promise.resolve();
}

/**
 * Move a steady window to a new layout.
 *
 * When only the position changes (same well side, another display spot) the
 * frame moves and the drawing is untouched: nothing can jump. When the drawing
 * changes too (the bar landed on the other side or half of the display, so it
 * now grows the other way, or the drag shrinks the window to the shape) the
 * frame and the page cannot be changed in the same display frame, which is the
 * very seam the steady frame exists to avoid. That swap is done hidden: the
 * content is made invisible, the frame and the drawing change, and it comes
 * back once both have reached the screen. It happens only on a drag, a drop or
 * a display hop, never on a state change.
 *
 * Swaps on one window run one after another, never interleaved, and the
 * "same drawing" check is made against the layout in force when this swap's
 * turn comes, not when it was asked for.
 */
export function swapSteadyLayout(
  label: string,
  next: SteadyLayout,
  applyFrame: ApplyFrame,
): Promise<void> {
  const run = steadySwapsSettled(label).then(() => runSwap(label, next, applyFrame));
  // The queue carries on past a failed swap; the caller still sees the error.
  const tail = run.catch(() => {});
  swapTails.set(label, tail);
  void tail.then(() => {
    if (swapTails.get(label) === tail) swapTails.delete(label);
  });
  return run;
}

async function runSwap(
  label: string,
  next: SteadyLayout,
  applyFrame: ApplyFrame,
): Promise<void> {
  const prev = getSteady(label)?.layout ?? null;
  if (prev && sameDrawing(prev, next)) {
    setSteadyLayout(label, (await applyFrame(next)) || next);
    return;
  }
  setSteadyHidden(label, true);
  try {
    await afterPaint();
    setSteadyLayout(label, (await applyFrame(next)) || next);
    await afterPaint();
  } finally {
    setSteadyHidden(label, false);
  }
}
