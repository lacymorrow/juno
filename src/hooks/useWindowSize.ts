import { useCallback } from 'react';
import { Window, currentMonitor } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

/** Which horizontal edge of the window stays put across a resize. */
export type WindowAnchorX = "start" | "center" | "end";

/** Where the pinned point sits in one frame of the window. */
export interface WindowAnchor {
  /**
   * Logical offset of the pinned point from the window's near edge: the top,
   * or the bottom when `growUp`.
   */
  anchorY: number;
  /** The near edge is the bottom: the window grows and shrinks upward. */
  growUp?: boolean;
}

export interface WindowSizeConfig {
  width: number;
  height: number;
  /**
   * Logical offset, from the near edge of the NEW frame, of the point that
   * must stay put on screen across the resize (the bar pins its pill band's
   * near edge here). Omitted: the top edge stays put.
   */
  anchorY?: number;
  /**
   * Grow upward instead of downward: the near edge is the bottom. A bar docked
   * in the bottom half of its display opens its pane above the pill, so the
   * added height extends up while the pill stays where it is.
   */
  growUp?: boolean;
  /**
   * The same point in the frame the window has RIGHT NOW. The live frame plus
   * this offset is the whole baseline, so nothing is remembered between
   * resizes and nothing can go stale when the window is moved by something
   * other than a resize (a glide into a gravity well, a display hop, the
   * launch restore). Defaults to the new frame's own offsets, which is right
   * whenever the pinned point is at the same offset before and after.
   */
  from?: WindowAnchor;
  /**
   * Which horizontal edge stays put. A bar docked in a left-hand well grows
   * rightward, one in a right-hand well grows leftward, so it never runs off
   * the screen edge it sits against. Default: the centre.
   */
  anchorX?: WindowAnchorX;
}

// Last applied size per window, so a resize to the size the window already
// has is skipped. `growUp` is part of it: a same-size resize that flips the
// growth direction moves the window and must go through.
const lastSizeByLabel: Map<
  string,
  { width: number; height: number; growUp: boolean }
> = new Map();

// Resizes are serialised per window. Each one reads the live frame and then
// writes a new one; two in flight at once would let the second read the frame
// before the first had landed and move the window from a position it no longer
// has. The bar's grow-then-animate protocol also relies on the promise
// resolving only once the frame is really applied.
const queueByLabel: Map<string, Promise<void>> = new Map();

function enqueue(label: string, op: () => Promise<void>): Promise<void> {
  const prev = queueByLabel.get(label) ?? Promise.resolve();
  const next = prev.then(op, op);
  queueByLabel.set(label, next);
  return next;
}

/** What the last resize left behind: the physical height and the anchor. */
interface AnchorState {
  /** Physical height of the frame. */
  physH: number;
  /** Logical offset of the pinned point from the frame's near edge. */
  anchor: number;
  /** Whether that frame's near edge is the bottom. */
  growUp: boolean;
}

/**
 * The new physical top edge for a resize that keeps one point at the same
 * screen position, whichever direction the window grows.
 *
 * `anchor` is the pinned point's distance from the frame's *near* edge (top
 * when growing down, bottom when growing up), so its distance from the top is
 * `anchor` (down) or `physH - anchor` (up). Keeping `top + that distance`
 * constant across the resize both pins the point and moves the top up when a
 * growUp window gains height. Falls back to a top-anchored resize when there
 * is no previous state or no anchor.
 */
export function anchoredTop(
  prevTop: number,
  prev: AnchorState | undefined,
  next: { physH: number; anchor?: number; growUp?: boolean },
  scale: number,
): number {
  if (prev === undefined || next.anchor === undefined) return prevTop;
  const fromTop = (physH: number, anchor: number, growUp: boolean) =>
    growUp ? physH - Math.round(anchor * scale) : Math.round(anchor * scale);
  const fOld = fromTop(prev.physH, prev.anchor, prev.growUp);
  const fNew = fromTop(next.physH, next.anchor, next.growUp ?? false);
  return prevTop + fOld - fNew;
}

/**
 * The new physical left edge for a resize that keeps one horizontal edge put:
 * the left edge, the right edge, or (default) the centre.
 */
export function anchoredLeft(
  prevX: number,
  prevW: number,
  nextW: number,
  anchorX: WindowAnchorX = "center",
): number {
  const dx = nextW - prevW;
  if (dx === 0) return prevX;
  switch (anchorX) {
    case "start":
      return prevX;
    case "end":
      return prevX - dx;
    default:
      return Math.round(prevX - dx / 2);
  }
}

/**
 * Edge-stable resize: the window's position is adjusted so the anchored edges
 * stay put. Without this, macOS resizes from the top-left, so a centred pill
 * would jump horizontally and a pane opening upward would run off the bottom.
 *
 * Uses physical pixel coordinates to match outerPosition()/outerSize() units.
 */
async function edgeStableResize(appWindow: Window, next: WindowSizeConfig) {
  const scaleFactor = await appWindow.scaleFactor();
  const physNextW = Math.round(next.width * scaleFactor);
  const physNextH = Math.round(next.height * scaleFactor);

  const pos = await appWindow.outerPosition();   // PhysicalPosition
  const size = await appWindow.outerSize();       // PhysicalSize

  const newX = anchoredLeft(pos.x, size.width, physNextW, next.anchorX);

  // The live frame is the baseline, whatever moved the window there.
  const from = next.from ?? { anchorY: next.anchorY, growUp: next.growUp };
  const prevAnchor: AnchorState | undefined =
    from.anchorY !== undefined
      ? { physH: size.height, anchor: from.anchorY, growUp: from.growUp ?? false }
      : undefined;
  const anchoredY = anchoredTop(
    pos.y,
    prevAnchor,
    { physH: physNextH, anchor: next.anchorY, growUp: next.growUp },
    scaleFactor,
  );

  const clamped = await clampToMonitor(newX, anchoredY, physNextW, physNextH);

  // Apply the move + resize atomically through the backend (a single macOS
  // NSWindow setFrame:). Issuing setPosition and setSize separately let the
  // WindowServer composite them in different frames, so for one frame the
  // window showed its new width still anchored at the old top-left and the
  // centered pill/dot visibly jumped before snapping back. One transaction
  // removes that seam. (Off macOS the command falls back to separate setters.)
  await invoke("set_bar_frame", {
    x: clamped.x,
    y: clamped.y,
    width: next.width,
    height: next.height,
  });
}

/**
 * Keep a resized window on its current monitor. A window growing downward (the
 * bar opening its chat pane) must not run off the bottom; an edge-docked bar
 * whose pane widens must not run off the left or right. Each axis is left
 * untouched when it already fits, otherwise nudged just enough to fit, never
 * past the monitor's opposite edge. Falls back to the input position when
 * monitor info is unavailable (tests, headless).
 */
async function clampToMonitor(
  x: number,
  y: number,
  physW: number,
  physH: number,
): Promise<{ x: number; y: number }> {
  try {
    const monitor = await currentMonitor();
    if (!monitor) return { x, y };
    const { position, size } = monitor;
    const right = position.x + size.width;
    const bottom = position.y + size.height;
    const clampedX =
      x + physW <= right ? Math.max(position.x, x) : Math.max(position.x, right - physW);
    const clampedY =
      y + physH <= bottom ? Math.max(position.y, y) : Math.max(position.y, bottom - physH);
    return { x: clampedX, y: clampedY };
  } catch {
    return { x, y };
  }
}

export function useWindowSize(windowLabel: string) {
  const resizeWindow = useCallback(
    (config: WindowSizeConfig) =>
      enqueue(windowLabel, async () => {
        try {
          const appWindow = await Window.getByLabel(windowLabel);
          if (appWindow) {
            await edgeStableResize(appWindow, config);
          }
        } catch (error) {
          console.error(`Failed to resize window ${windowLabel}:`, error);
        }
      }),
    [windowLabel],
  );

  const resizeWindowIfChanged = useCallback(
    (config: WindowSizeConfig) =>
      enqueue(windowLabel, async () => {
        try {
          const growUp = config.growUp ?? false;
          const prev = lastSizeByLabel.get(windowLabel);
          if (
            prev &&
            prev.width === config.width &&
            prev.height === config.height &&
            prev.growUp === growUp
          ) {
            return; // no-op
          }

          const appWindow = await Window.getByLabel(windowLabel);
          if (appWindow) {
            await edgeStableResize(appWindow, config);
            lastSizeByLabel.set(windowLabel, {
              width: config.width,
              height: config.height,
              growUp,
            });
          }
        } catch (error) {
          console.error(`Failed to resize window ${windowLabel}:`, error);
        }
      }),
    [windowLabel],
  );

  const getWindowSize = useCallback(async (): Promise<WindowSizeConfig | null> => {
    try {
      const appWindow = await Window.getByLabel(windowLabel);
      if (appWindow) {
        const size = await appWindow.innerSize();
        return {
          width: size.width,
          height: size.height,
        };
      }
      return null;
    } catch (error) {
      console.error(`Failed to get window size for ${windowLabel}:`, error);
      return null;
    }
  }, [windowLabel]);

  return {
    resizeWindow,
    resizeWindowIfChanged,
    getWindowSize,
  };
}
