/**
 * Snap wells for the floating bar.
 *
 * The bar drags freely, but on release it settles into the nearest of a small
 * set of tidy "wells" laid out around every connected display — the corners
 * and edge-midpoints, inset from the screen edges. Wispr Flow snaps to a
 * couple of spots; superwhisper to ~20; this gives eight per display (a 3×3
 * grid minus the dead centre), which is dense enough to feel free but always
 * lands somewhere deliberate.
 *
 * Everything here is pure and works in **physical** pixels, matching Tauri's
 * `outerPosition()` / monitor geometry / `PhysicalPosition`, so it is unit
 * tested without a running window. Margins are given in logical pixels and
 * scaled per-monitor by its `scaleFactor`.
 */

export interface MonitorRect {
  position: { x: number; y: number };
  size: { width: number; height: number };
  scaleFactor: number;
}

export type WellCol = "left" | "center" | "right";
export type WellRow = "top" | "middle" | "bottom";

export interface Well {
  /** Target for the window's top-left, in physical pixels. */
  x: number;
  y: number;
  col: WellCol;
  row: WellRow;
  monitorIndex: number;
}

export interface WellOptions {
  /** Current window size, in physical pixels. */
  windowWidth: number;
  windowHeight: number;
  /** Inset from the left/right/bottom edges, in logical pixels. */
  margin?: number;
  /** Inset from the top edge, in logical pixels — clears the menu bar. */
  topInset?: number;
  /** Include the dead-centre well (a bar mid-screen); off by default. */
  includeCenter?: boolean;
}

const COLS: { name: WellCol; f: number }[] = [
  { name: "left", f: 0 },
  { name: "center", f: 0.5 },
  { name: "right", f: 1 },
];
const ROWS: { name: WellRow; f: number }[] = [
  { name: "top", f: 0 },
  { name: "middle", f: 0.5 },
  { name: "bottom", f: 1 },
];

/**
 * The wells for every monitor, as top-left targets for a window of the given
 * size. When the window is larger than a monitor's inset area (a wide pane on
 * a small screen), the corresponding axis collapses to the inset origin.
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

    const left = m.position.x + marginP;
    const right = m.position.x + m.size.width - marginP;
    const top = m.position.y + topInsetP;
    const bottom = m.position.y + m.size.height - marginP;

    // Range available for the window's top-left within the inset area.
    const spanX = Math.max(0, right - windowWidth - left);
    const spanY = Math.max(0, bottom - windowHeight - top);

    for (const row of ROWS) {
      for (const col of COLS) {
        if (!includeCenter && col.f === 0.5 && row.f === 0.5) continue;
        wells.push({
          x: Math.round(left + col.f * spanX),
          y: Math.round(top + row.f * spanY),
          col: col.name,
          row: row.name,
          monitorIndex,
        });
      }
    }
  });

  return wells;
}

/** Squared distance — enough for choosing the nearest, no sqrt needed. */
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

/** Ease-out cubic, for the settle animation. */
export function easeOutCubic(t: number): number {
  const c = 1 - t;
  return 1 - c * c * c;
}
