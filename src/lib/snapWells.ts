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
 * Everything here is pure and works in **physical** pixels for positions,
 * matching Tauri's `outerPosition()` / monitor geometry / `PhysicalPosition`,
 * so it is unit tested without a running window. The window size and the
 * margins are given in **logical** pixels and scaled per monitor by its
 * `scaleFactor`: a window keeps its logical size when it moves between a
 * Retina and a 1× display, so its physical footprint changes, and a well
 * computed with the wrong footprint pushes the bar off the far edge.
 */

export interface MonitorRect {
  position: { x: number; y: number };
  size: { width: number; height: number };
  scaleFactor: number;
}

/** Where a well sits on its display: fraction along each axis, 0..1. */
export interface WellSlot {
  fx: number;
  fy: number;
}

export interface Well extends WellSlot {
  /** Target for the window's top-left, in physical pixels. */
  x: number;
  y: number;
  /** The window's footprint on this well's display, in physical pixels. */
  width: number;
  height: number;
  monitorIndex: number;
}

export interface WellOptions {
  /** Current window size, in logical pixels. */
  windowWidth: number;
  windowHeight: number;
  /** Inset from the left/right/bottom edges, in logical pixels. */
  margin?: number;
  /** Inset from the top edge, in logical pixels: clears the menu bar. */
  topInset?: number;
  /** Include the dead-centre well (a bar mid-screen); off by default. */
  includeCenter?: boolean;
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
 * A display this wide (or tall), in logical pixels, gets five stops along
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
 * logical size. When the window is larger than a monitor's inset area (a wide
 * pane on a small screen), the corresponding axis collapses to the inset origin.
 */
export function computeWells(
  monitors: MonitorRect[],
  {
    windowWidth,
    windowHeight,
    margin = 16,
    topInset = 36,
    includeCenter = false,
  }: WellOptions,
): Well[] {
  const wells: Well[] = [];

  monitors.forEach((m, monitorIndex) => {
    const sf = m.scaleFactor || 1;
    const marginP = margin * sf;
    const topInsetP = topInset * sf;
    const width = Math.round(windowWidth * sf);
    const height = Math.round(windowHeight * sf);

    const left = m.position.x + marginP;
    const right = m.position.x + m.size.width - marginP;
    const top = m.position.y + topInsetP;
    const bottom = m.position.y + m.size.height - marginP;

    // Range available for the window's top-left within the inset area.
    const spanX = Math.max(0, right - width - left);
    const spanY = Math.max(0, bottom - height - top);

    const xs = axisStops(m.size.width / sf);
    const ys = axisStops(m.size.height / sf);

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
