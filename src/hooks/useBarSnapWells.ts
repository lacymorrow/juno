/**
 * Gravity wells for the bar window, whichever look is in it.
 *
 * The bar drags freely, but on release it glides into the nearest **well**:
 * one of the tidy anchor points around every display (corners, edge-midpoints
 * and the centre, inset from the edges, see `src/lib/snapWells.ts`). The user
 * aims roughly; the wells make it land deliberately.
 *
 * This used to live inside FloatingBar, so only the Pill had it. Every other
 * look called the drag hook and dragged, then stayed wherever the OS dropped
 * it. The snap lives here instead, driven from the one drag gesture in
 * `useDragWindow`, so a look added next month gets wells the moment it calls
 * the drag hook it would call anyway.
 *
 * The Rust/React line: React decides which well (pure maths over monitor
 * rects in global points), Rust performs the moves, and drives the drag of a
 * steady look itself (`platform/bar_drag.rs`).
 *
 * Only the bar window snaps. `useDragWindow` also drags the floating panels,
 * which have no wells and must keep dragging without them, so every entry
 * point here is gated on the current window's label.
 */


import { useCallback, useMemo, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import {
  availableMonitors,
  cursorPosition,
  getCurrentWindow,
  LogicalPosition,
} from "@tauri-apps/api/window";

import { COMMANDS, EVENTS, WINDOW_LABELS } from "@/lib/constants.generated";
import {
  computeWells,
  easeOutCubic,
  nearestWell,
  wellForSlot,
  type MonitorRect,
  type Well,
  type WellSlot,
} from "@/lib/snapWells";
import {
  getDockSlot,
  monitorIndexAt,
  predictedWindowOrigin,
  setDockSlot,
  subscribeDockSlot,
} from "@/lib/barDock";
import {
  cursorInPoints,
  monitorsInPoints,
  windowOriginInPoints,
  type TauriMonitorLike,
} from "@/lib/desktopPoints";
import {
  dragLayout,
  getSteady,
  steadyLayout,
  swapSteadyLayout,
  type SteadyLayout,
  type SteadySpec,
} from "@/lib/steadyFrame";
import { useEventListener } from "./useEventListener";

/** Settle animation: min/max duration, and the travel below which it's skipped. */
export const SNAP_MIN_MS = 160;
export const SNAP_MAX_MS = 340;
export const SNAP_MIN_TRAVEL_PX = 2;

type AppWindow = ReturnType<typeof getCurrentWindow>;

/** The current window's label, or "" when there is no Tauri window at all. */
export function currentWindowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "";
  }
}

/** Wells belong to the bar window only; the floating panels just drag. */
export function isBarWindow(): boolean {
  return currentWindowLabel() === WINDOW_LABELS.FLOATING_BAR;
}

/** Tauri monitors as the rects the well math takes: global points. */
export function toMonitorRects(mons: TauriMonitorLike[]): MonitorRect[] {
  return monitorsInPoints(mons);
}

/**
 * The window's size in points. Wells are computed from this every time, so
 * the geometry rule needs no per-look data: a narrow Pill and a full-width
 * Studio strip go through exactly the same code.
 */
export async function logicalWindowSize(win: AppWindow) {
  const [size, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
  return { windowWidth: size.width / scale, windowHeight: size.height / scale };
}

/** The window's top-left in global points. */
export async function windowOrigin(win: AppWindow): Promise<{ x: number; y: number }> {
  const [pos, scale] = await Promise.all([win.outerPosition(), win.scaleFactor()]);
  return windowOriginInPoints(pos, scale);
}

/**
 * Animate a window's top-left from `from` to `to` (global points) with an
 * ease-out, so a released bar glides into its well instead of teleporting.
 * A magnet, not a trampoline: no overshoot, no bounce.
 */
export async function animateWindowTo(
  win: AppWindow,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  if (Math.abs(dx) + Math.abs(dy) < SNAP_MIN_TRAVEL_PX) return;
  const duration = Math.min(
    SNAP_MAX_MS,
    Math.max(SNAP_MIN_MS, Math.hypot(dx, dy) * 0.35),
  );
  const start = performance.now();
  await new Promise<void>((resolve) => {
    const step = () => {
      const t = Math.min(1, (performance.now() - start) / duration);
      const e = easeOutCubic(t);
      void win.setPosition(
        new LogicalPosition(Math.round(from.x + dx * e), Math.round(from.y + dy * e)),
      );
      if (t < 1) requestAnimationFrame(step);
      else resolve();
    };
    requestAnimationFrame(step);
  });
}

/**
 * Put a steady-frame window where its layout says, in one native transaction.
 * The size is the look's largest footprint and never changes per state; only
 * the position moves, and only when the well does.
 */
export async function applySteadyFrame(layout: SteadyLayout): Promise<void> {
  await invoke(COMMANDS.BAR_SET_BAR_FRAME, {
    x: layout.origin.x,
    y: layout.origin.y,
    width: layout.size.width,
    height: layout.size.height,
  });
}

/**
 * The steady layout for `well` on its display, or null when the display is
 * gone. Wells for a steady look are computed for its resting footprint.
 */
function steadyLayoutFor(
  well: Well,
  rects: MonitorRect[],
  spec: SteadySpec,
): SteadyLayout | null {
  const mon = rects[well.monitorIndex];
  return mon ? steadyLayout(well, mon, spec) : null;
}

/** Wells for a steady look: computed for the resting footprint, not the window. */
export function steadyWells(rects: MonitorRect[], spec: SteadySpec): Well[] {
  return computeWells(rects, {
    windowWidth: spec.rest.width,
    windowHeight: spec.rest.height,
    includeCenter: true,
  });
}

// ── The drag/settle controller ────────────────────────────────────────────
//
// Module state, not refs: there is one bar window, the gesture is one at a
// time, and both the drag hook and the window-level mouseup have to see the
// same arm flag. Set the moment a drag starts; consumed once on release.

let snapArmed = false;
let snapAnimating = false;
// Whether the drop indicator overlay is showing, so it is hidden exactly once
// on release regardless of which settle path fires first.
let overlayShown = false;
// What the overlays were last told to show, so an overlay window that mounts
// mid-drag (a display's first drag builds its overlay) can be told too.
let lastShowPayload: Record<string, number> | null = null;
// Where the press landed, in points from the footprint's top-left (the steady
// anchor, or the window itself). The settle reads the landing from the
// cursor with it, exactly as the drop indicator does.
let footprintGrab: { x: number; y: number } | null = null;
// Rust is moving the window (`bar_drag_follow`), so the settle must stop it.
let drivenDrag = false;
// The drag's start-up (the re-layout for the drag) while it is in flight. The
// settle waits for it so the two never interleave.
let dragStarting: Promise<void> | null = null;

/** Forget the in-flight gesture. For tests, which share one module instance. */
export function resetBarSnapState(): void {
  snapArmed = false;
  snapAnimating = false;
  overlayShown = false;
  lastShowPayload = null;
  footprintGrab = null;
  drivenDrag = false;
  dragStarting = null;
}

/** The cursor in global points, or null when it cannot be read. */
async function cursorPoints(mons: TauriMonitorLike[]): Promise<{ x: number; y: number } | null> {
  try {
    return cursorInPoints(await cursorPosition(), mons);
  } catch {
    return null;
  }
}

/**
 * Where the footprint's top-left would be if it followed the cursor exactly,
 * in global points. This picks the well, so the landing agrees with the drop
 * indicator, which predicts from the cursor the same way. Null when the
 * cursor cannot be read; the caller falls back to the window position.
 */
async function releasedFootprintOrigin(
  mons: TauriMonitorLike[],
): Promise<{ x: number; y: number } | null> {
  if (!footprintGrab) return null;
  const c = await cursorPoints(mons);
  return c ? predictedWindowOrigin(c, footprintGrab) : null;
}

/**
 * A drag has just started: arm the settle and show the drop indicator for the
 * length of the drag.
 *
 * `grabOffset` is where inside the window the press landed, in points (a DOM
 * `clientX`/`clientY`). It rides along in the show payload so the overlay can
 * predict the footprint's top-left from the cursor and brighten the well the
 * bar will actually land in.
 */
export async function armBarSnap(grabOffset: { x: number; y: number }): Promise<void> {
  if (!isBarWindow()) return;
  snapArmed = true;
  if (overlayShown) return;
  footprintGrab = grabOffset;
  overlayShown = true;
  // One overlay per display. Building a missing one is slow and must not hold
  // up the others; a new one asks for the payload once it is listening.
  void invoke(COMMANDS.BAR_ENSURE_SNAP_WELLS_OVERLAYS).catch((error) =>
    console.debug("barSnap: could not prepare the overlays:", error),
  );
  try {
    const win = getCurrentWindow();
    // A steady look's window is mostly empty room; the wells, the prediction
    // and the ring are all about its resting footprint (the anchor rect), so
    // the grab is re-expressed from that footprint's top-left.
    const steady = getSteady(win.label);
    if (steady?.layout) {
      footprintGrab = {
        x: grabOffset.x - steady.layout.anchor.x,
        y: grabOffset.y - steady.layout.anchor.y,
      };
      lastShowPayload = {
        windowWidth: steady.spec.rest.width,
        windowHeight: steady.spec.rest.height,
        grabOffsetX: footprintGrab.x,
        grabOffsetY: footprintGrab.y,
      };
    } else {
      const logical = await logicalWindowSize(win);
      lastShowPayload = { ...logical, grabOffsetX: grabOffset.x, grabOffsetY: grabOffset.y };
    }
    await emit(EVENTS.SNAP_WELLS_SHOW, lastShowPayload);
  } catch (error) {
    console.debug("barSnap: snap-wells-show failed:", error);
  }
}

/** An overlay window has started listening: if a drag is on, tell it what to draw. */
export function answerOverlayReady(): void {
  if (!overlayShown || !lastShowPayload) return;
  void emit(EVENTS.SNAP_WELLS_SHOW, lastShowPayload).catch(() => {});
}

/**
 * A press on the bar has become a drag. Arms the snap and moves the window.
 *
 * A steady look is dragged by Rust (`bar_drag_follow`), not by the OS. The OS
 * drag keeps a window's top edge below the menu bar, and a steady window is
 * far taller than the shape in it, so a pill docked low could not be carried
 * above the middle of the screen. Rust moves the window itself, unconstrained,
 * so the shape follows the cursor anywhere. The drawing is then re-laid out
 * with the shape centred in its window (`dragLayout`), so that crossing onto
 * another display the window changes display with the shape rather than
 * hundreds of points later. Every other look, and any platform where the
 * driven drag is unavailable, uses the OS drag as before.
 */
export function startBarDrag(grabOffset: { x: number; y: number }): void {
  void armBarSnap(grabOffset);
  const win = getCurrentWindow();
  const osDrag = () =>
    win.startDragging().catch((error) => console.debug("barDrag: startDragging failed:", error));
  const layout = isBarWindow() ? getSteady(win.label)?.layout : null;
  if (!layout) {
    void osDrag();
    return;
  }
  dragStarting = (async () => {
    try {
      await invoke(COMMANDS.BAR_DRAG_FOLLOW, { grabX: grabOffset.x, grabY: grabOffset.y });
      drivenDrag = true;
    } catch {
      await osDrag();
      return;
    }
    const drag = dragLayout(layout);
    const grabInDrag = {
      x: drag.anchor.x + grabOffset.x - layout.anchor.x,
      y: drag.anchor.y + grabOffset.y - layout.anchor.y,
    };
    try {
      // The window size does not change, only where the shape sits in it, so
      // "applying the frame" is moving the grab: Rust's next tick puts the
      // window where the centred drawing keeps the shape under the cursor.
      await swapSteadyLayout(win.label, drag, () =>
        invoke(COMMANDS.BAR_DRAG_FOLLOW, { grabX: grabInDrag.x, grabY: grabInDrag.y }).then(
          () => {},
        ),
      );
    } catch (error) {
      console.debug("barDrag: drag layout failed:", error);
    }
  })();
}

/**
 * On release after a drag, glide the bar into the nearest well and remember
 * the slot it landed in: the well decides the growth direction and the
 * anchored column from here on, and where the bar reopens next launch.
 */
export async function settleBarSnap(): Promise<void> {
  // Hide the drop indicator on any release path, even if the settle below
  // no-ops (drag not armed, or already consumed by another path).
  if (overlayShown) {
    overlayShown = false;
    lastShowPayload = null;
    void emit(EVENTS.SNAP_WELLS_HIDE);
  }
  if (!snapArmed || snapAnimating) return;
  snapArmed = false;
  snapAnimating = true;
  try {
    if (dragStarting) {
      await dragStarting;
      dragStarting = null;
    }
    if (drivenDrag) {
      drivenDrag = false;
      await invoke(COMMANDS.BAR_DRAG_STOP).catch(() => {});
    }
    const win = getCurrentWindow();
    const steady = getSteady(win.label);
    if (steady?.layout) {
      await settleSteady(win, steady.spec, steady.layout);
      return;
    }
    const [origin, logical, monitors] = await Promise.all([
      windowOrigin(win),
      logicalWindowSize(win),
      availableMonitors(),
    ]);
    if (!monitors.length) return;
    const rects = toMonitorRects(monitors);
    const wells = computeWells(rects, { ...logical, includeCenter: true });
    const aim = (await releasedFootprintOrigin(monitors)) ?? origin;
    const target = nearestWell(aim, wells);
    if (!target) return;
    await animateWindowTo(win, origin, { x: target.x, y: target.y });
    setDockSlot(win.label, { fx: target.fx, fy: target.fy });
    try {
      await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y });
    } catch (error) {
      console.debug("barSnap: persist well failed:", error);
    }
  } catch (error) {
    console.debug("barSnap: settle into well failed:", error);
  } finally {
    snapAnimating = false;
    footprintGrab = null;
  }
}

/**
 * The settle for a steady look. The window glides, drawing unchanged, until
 * the resting footprint sits on the nearest well; then, if the well's layout
 * draws differently (the drag layout always does), the frame and the drawing
 * swap behind a brief hide (`swapSteadyLayout`). The well's own position
 * never changes in that swap, so the bar lands exactly where the glide put it.
 */
async function settleSteady(win: AppWindow, spec: SteadySpec, layout: SteadyLayout) {
  const [origin, monitors] = await Promise.all([windowOrigin(win), availableMonitors()]);
  if (!monitors.length) return;
  const rects = toMonitorRects(monitors);
  const anchorNow = (await releasedFootprintOrigin(monitors)) ?? {
    x: origin.x + layout.anchor.x,
    y: origin.y + layout.anchor.y,
  };
  const target = nearestWell(anchorNow, steadyWells(rects, spec));
  if (!target) return;
  const next = steadyLayoutFor(target, rects, spec);
  if (!next) return;
  await animateWindowTo(win, origin, {
    x: target.x - layout.anchor.x,
    y: target.y - layout.anchor.y,
  });
  await swapSteadyLayout(win.label, next, applySteadyFrame);
  setDockSlot(win.label, { fx: target.fx, fy: target.fy });
  try {
    await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y });
  } catch (error) {
    console.debug("barSnap: persist well failed:", error);
  }
}

/** True while a drag is armed or the glide is still running. */
export function barSnapBusy(): boolean {
  return snapArmed || snapAnimating;
}

// ── Hooks ─────────────────────────────────────────────────────────────────

/**
 * The well this window is docked in. Every look reads it to lay itself out
 * against the docked edges, and `useWindowSize` reads it to anchor resizes on
 * the same edges, so the two cannot disagree.
 */
export function useDockSlot(): WellSlot | null {
  const label = useMemo(() => currentWindowLabel(), []);
  const subscribe = useCallback(
    (fn: () => void) => subscribeDockSlot(label, fn),
    [label],
  );
  const snapshot = useCallback(() => getDockSlot(label), [label]);
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}

/**
 * Where the bar re-homes when the cursor moves to another display, or null
 * when it stays. Pure, so the hop is unit tested on mixed desks: the same
 * slot on the cursor's display, when the bar is not already there. All in
 * global points.
 */
export function displayFollowTarget(
  rects: MonitorRect[],
  slot: WellSlot,
  cursor: { x: number; y: number },
  currentDisplay: number,
  wells: Well[],
): Well | null {
  const targetIdx = monitorIndexAt(rects, cursor.x, cursor.y);
  if (targetIdx < 0 || targetIdx === currentDisplay) return null;
  return wellForSlot(slot, targetIdx, wells);
}

/**
 * Follow the cursor between displays: when the backend reports the cursor has
 * moved to another display, re-home the bar to the same well slot there, so it
 * is always where the user is looking.
 *
 * The slot is what survives the hop, not the coordinates: the same corner with
 * the same padding, on a display of its own size. `paused` is how a look says
 * it is busy (the Pill pauses while its chat pane is open or the agent is
 * working); a dragging or gliding bar is never moved. The payload is the
 * cursor in global points.
 */
export function useBarDisplayFollow(paused = false): void {
  const onCursorDisplayChange = useCallback(
    async (cursor: { x: number; y: number }) => {
      if (paused || !isBarWindow()) return;
      if (barSnapBusy()) return;
      const win = getCurrentWindow();
      const slot = getDockSlot(win.label);
      if (!slot) return;
      const steady = getSteady(win.label);
      try {
        const mons = await availableMonitors();
        if (!mons.length) return;
        const rects = toMonitorRects(mons);
        if (steady?.layout) {
          const target = displayFollowTarget(
            rects,
            slot,
            cursor,
            steady.layout.monitorIndex,
            steadyWells(rects, steady.spec),
          );
          if (!target) return;
          const next = steadyLayoutFor(target, rects, steady.spec);
          if (!next) return;
          await swapSteadyLayout(win.label, next, applySteadyFrame);
          setDockSlot(win.label, { fx: target.fx, fy: target.fy });
          await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(
            () => {},
          );
          return;
        }
        const [logical, origin] = await Promise.all([logicalWindowSize(win), windowOrigin(win)]);
        const target = displayFollowTarget(
          rects,
          slot,
          cursor,
          monitorIndexAt(rects, origin.x, origin.y),
          computeWells(rects, { ...logical, includeCenter: true }),
        );
        if (!target) return;
        await win.setPosition(new LogicalPosition(target.x, target.y));
        setDockSlot(win.label, { fx: target.fx, fy: target.fy });
        await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(
          () => {},
        );
      } catch (error) {
        console.debug("barSnap: cursor-follow move failed:", error);
      }
    },
    [paused],
  );

  // `useEventListener` keeps the latest handler in a ref, so the listener is
  // registered once and still sees the current `paused`.
  useEventListener<{ x: number; y: number }>(
    EVENTS.BAR_CURSOR_DISPLAY_CHANGED,
    (payload) => void onCursorDisplayChange(payload),
  );
}
