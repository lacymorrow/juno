/**
 * Snap wells for the floating bar.
 *
 * The bar drags freely, but on release it settles into the nearest of a small
 * set of tidy "wells" laid out around every connected display: the corners,
 * the edge-midpoints and (when enabled) the centre, inset from the screen
 * edges. A laptop screen gets a 3×3 grid; a wide or tall display gets five
 * stops along its long axis, so an ultrawide offers the quarter points and
 * not just left / centre / right.
 *
 * Every well carries its **slot**, the fraction of the way along each axis
 * (`fx`, `fy` in 0..1). Slots are what survive a display change: the same
 * slot on another display is the same place, "bottom-left corner with the
 * padding", whatever that display's size, aspect ratio or pixel density.
 *
 * Everything here is pure and works in **global desktop points**: the one
 * coordinate space macOS lays every display out in, top-left origin at the
 * primary display, y down, the same unit CSS pixels and NSScreen frames use.
 * It is the only space that is consistent across displays. Tauri's "physical"
 * positions are not: each monitor's origin is its point origin multiplied by
 * its OWN scale factor, the cursor is multiplied by the PRIMARY display's, and
 * a window's by the display it is on. With a 1x display beside a 2x Retina
 * those disagree by a factor of two, which is what scattered the wells on a
 * second display. `src/lib/desktopPoints.ts` converts Tauri's numbers into
 * points at the edge; nothing past it sees a physical pixel.
 */

/** A display's frame in global desktop points. */
export interface MonitorRect {
  position: { x: number; y: number };
  size: { width: number; height: number };
  /** The display's backing scale. Informational: the geometry never uses it. */
  scaleFactor?: number;
  /**
   * The part of the display the menu bar and the Dock leave free
   * (`NSScreen.visibleFrame`), in global points. Absent in the harness and in
   * tests that do not care, which then get the fixed insets.
   */
  workArea?: {
    position: { x: number; y: number };
    size: { width: number; height: number };
  };
}

/** Where a well sits on its display: fraction along each axis, 0..1. */
export interface WellSlot {
  fx: number;
  fy: number;
}

export interface Well extends WellSlot {
  /** Target for the window's top-left, in global points. */
  x: number;
  y: number;
  /** The window's footprint, in points. */
  width: number;
  height: number;
  monitorIndex: number;
}

export interface WellOptions {
  /** Current window size, in points. */
  windowWidth: number;
  windowHeight: number;
  /** Inset from the left/right/bottom edges, in points, beyond the work area. */
  margin?: number;
  /** Inset from the top edge when the display reports no work area. */
  topInset?: number;
  /** Include the dead-centre well (a bar mid-screen); off by default. */
  includeCenter?: boolean;
}

/** Breathing room from the left, right and bottom edges (and the Dock), in points. */
export const WELL_MARGIN = 16;
/** Room between the menu bar and a top-row well, in points. */
export const MENU_BAR_GAP = 12;
/**
 * Top inset when a display reports no work area: a standard 24 point menu bar
 * plus the gap. Only the harness and old tests see it.
 */
export const FALLBACK_TOP_INSET = 24 + MENU_BAR_GAP;

/** How far in from each edge of a display the wells (and steady windows) sit. */
export interface EdgeInsets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/**
 * The insets for one display, from its real work area.
 *
 * The menu bar is not a constant: 24 points on a display without a notch,
 * 37 or 38 on a notched MacBook, 0 when it auto-hides. macOS also keeps a
 * floating window's top edge below the menu bar whenever it sets the frame,
 * so a well that asked for y = 36 under a 38 point menu bar was silently put
 * at 38, two points off its well. Reading the work area puts the top row just
 * under whatever menu bar this display has, and keeps the other rows clear of
 * the Dock (which sits above the bar's level and would cover it).
 */
export function screenInsets(
  m: MonitorRect,
  { margin = WELL_MARGIN, topInset = FALLBACK_TOP_INSET }: { margin?: number; topInset?: number } = {},
): EdgeInsets {
  const wa = m.workArea;
  if (!wa) return { top: topInset, right: margin, bottom: margin, left: margin };
  const menuBar = Math.max(0, wa.position.y - m.position.y);
  const left = Math.max(0, wa.position.x - m.position.x);
  const right = Math.max(0, m.position.x + m.size.width - (wa.position.x + wa.size.width));
  const bottom = Math.max(0, m.position.y + m.size.height - (wa.position.y + wa.size.height));
  return {
    // No menu bar on this display (auto-hide): the ordinary margin.
    top: menuBar > 0 ? menuBar + MENU_BAR_GAP : margin,
    right: right + margin,
    bottom: bottom + margin,
    left: left + margin,
  };
}

/** Corner and edge slots for the default well, by name. */
export const SLOT = {
  topLeft: { fx: 0, fy: 0 },
  topRight: { fx: 1, fy: 0 },
  bottomLeft: { fx: 0, fy: 1 },
  bottomRight: { fx: 1, fy: 1 },
  center: { fx: 0.5, fy: 0.5 },
} as const satisfies Record<string, WellSlot>;

/**
 * A display this wide (or tall), in points, gets five stops along
 * that axis instead of three. 1440 and 1920 wide screens stay at three;
 * 2560 (an ultrawide, a 27" at 2× "looks like" 2560) gets five.
 */
export const FIVE_STOP_MIN_LOGICAL = 2000;

/** Evenly spaced fractions along one axis: 0, …, 1. Always odd, so 0.5 exists. */
export function axisStops(logicalSpan: number): number[] {
  const count = logicalSpan >= FIVE_STOP_MIN_LOGICAL ? 5 : 3;
  return Array.from({ length: count }, (_, i) => i / (count - 1));
}

/**
 * The wells for every monitor, as top-left targets for a window of the given
 * size, all in points, inside each display's `screenInsets`. When the window is larger than a monitor's inset area (a wide
 * pane on a small screen), the corresponding axis collapses to the inset origin.
 */
export function computeWells(
  monitors: MonitorRect[],
  {
    windowWidth,
    windowHeight,
    margin = WELL_MARGIN,
    topInset = FALLBACK_TOP_INSET,
    includeCenter = false,
  }: WellOptions,
): Well[] {
  const wells: Well[] = [];

  monitors.forEach((m, monitorIndex) => {
    const width = windowWidth;
    const height = windowHeight;

    const inset = screenInsets(m, { margin, topInset });
    const left = m.position.x + inset.left;
    const right = m.position.x + m.size.width - inset.right;
    const top = m.position.y + inset.top;
    const bottom = m.position.y + m.size.height - inset.bottom;

    // Range available for the window's top-left within the inset area.
    const spanX = Math.max(0, right - width - left);
    const spanY = Math.max(0, bottom - height - top);

    const xs = axisStops(m.size.width);
    const ys = axisStops(m.size.height);

    for (const fy of ys) {
      for (const fx of xs) {
        if (!includeCenter && fx === 0.5 && fy === 0.5) continue;
        wells.push({
          x: Math.round(left + fx * spanX),
          y: Math.round(top + fy * spanY),
          width,
          height,
          fx,
          fy,
          monitorIndex,
        });
      }
    }
  });

  return wells;
}

/** Squared distance: enough for choosing the nearest, no sqrt needed. */
function dist2(ax: number, ay: number, bx: number, by: number): number {
  const dx = ax - bx;
  const dy = ay - by;
  return dx * dx + dy * dy;
}

/**
 * The well whose top-left is closest to `point` (the window's current
 * top-left). Returns null when there are no wells.
 */
export function nearestWell(
  point: { x: number; y: number },
  wells: Well[],
): Well | null {
  let best: Well | null = null;
  let bestD = Infinity;
  for (const w of wells) {
    const d = dist2(point.x, point.y, w.x, w.y);
    if (d < bestD) {
      bestD = d;
      best = w;
    }
  }
  return best;
}

/**
 * The well on `monitorIndex` that is the same place as `slot`: an exact slot
 * match when the display has it, otherwise the closest fraction on each axis
 * (a quarter-point slot from a five-stop ultrawide lands on the nearer of
 * edge and centre on a three-stop laptop; ties go to the centre so the bar
 * stays in view). Returns null when the monitor has no wells.
 */
export function wellForSlot(
  slot: WellSlot,
  monitorIndex: number,
  wells: Well[],
): Well | null {
  let best: Well | null = null;
  let bestD = Infinity;
  for (const w of wells) {
    if (w.monitorIndex !== monitorIndex) continue;
    const d = dist2(slot.fx, slot.fy, w.fx, w.fy);
    // Prefer the slot nearer the centre on a tie, never the edge.
    const centre = dist2(0.5, 0.5, w.fx, w.fy);
    const bestCentre = best ? dist2(0.5, 0.5, best.fx, best.fy) : Infinity;
    if (d < bestD || (d === bestD && centre < bestCentre)) {
      bestD = d;
      best = w;
    }
  }
  return best;
}

/** Ease-out cubic, for the settle animation. */
export function easeOutCubic(t: number): number {
  const c = 1 - t;
  return 1 - c * c * c;
}
