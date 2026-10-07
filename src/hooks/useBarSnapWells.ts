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
 * The Rust/React line is unchanged: React decides which well (pure maths over
 * monitor rects), Rust performs the move through the existing commands.
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
  PhysicalPosition,
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

/** Tauri monitors as the plain rects the well math takes. */
export function toMonitorRects(
  mons: Array<{
    position: { x: number; y: number };
    size: { width: number; height: number };
    scaleFactor: number;
  }>,
): MonitorRect[] {
  return mons.map((m) => ({
    position: { x: m.position.x, y: m.position.y },
    size: { width: m.size.width, height: m.size.height },
    scaleFactor: m.scaleFactor,
  }));
}

/**
 * The window's size in logical pixels. Wells are computed from this, never
 * from the physical size, because the physical footprint changes with the
 * display's pixel density and a well computed for the wrong one lands the
 * bar off the far edge of a Retina screen.
 *
 * It is measured every time, so the geometry rule needs no per-look data: a
 * narrow Pill and a full-width Studio strip go through exactly the same code.
 */
export async function logicalWindowSize(win: AppWindow) {
  const [size, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
  return { windowWidth: size.width / scale, windowHeight: size.height / scale };
}

/**
 * Animate a window's top-left from `from` to `to` (physical px) with an
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
        new PhysicalPosition(
          Math.round(from.x + dx * e),
          Math.round(from.y + dy * e),
        ),
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
// same arm flag. Set the moment a drag hands off to the OS; consumed once on
// release.

let snapArmed = false;
let snapAnimating = false;
// Whether the drop indicator overlay is showing, so it is hidden exactly once
// on release regardless of which settle path fires first.
let overlayShown = false;
// Where the press landed, in logical px from the footprint's top-left (the
// steady anchor, or the window itself). The settle reads the landing from the
// cursor with it, exactly as the drop indicator does.
let footprintGrab: { x: number; y: number } | null = null;

/** Forget the in-flight gesture. For tests, which share one module instance. */
export function resetBarSnapState(): void {
  snapArmed = false;
  snapAnimating = false;
  overlayShown = false;
  footprintGrab = null;
}

/**
 * Where the footprint's top-left would be if the window had followed the
 * cursor exactly, in physical px. This, not the window's own position, picks
 * the well: macOS keeps a window's top edge below the menu bar during a drag,
 * and a steady window is far taller than the Pill inside it, so its position
 * alone could never reach the top row of wells. Null when the cursor cannot
 * be read; the caller falls back to the window position.
 */
async function releasedFootprintOrigin(
  rects: MonitorRect[],
): Promise<{ x: number; y: number } | null> {
  if (!footprintGrab) return null;
  try {
    const c = await cursorPosition();
    return predictedWindowOrigin({ x: c.x, y: c.y }, footprintGrab, rects);
  } catch {
    return null;
  }
}

/**
 * A drag has just handed off to the OS: arm the settle and show the drop
 * indicator overlay for the length of the drag.
 *
 * `grabOffset` is where inside the window the press landed, in logical px (a
 * DOM `clientX`/`clientY`). It rides along in the show payload so the overlay
 * can predict the window's top-left from the cursor and brighten the well the
 * bar will actually land in. Without it the overlay compared the bare cursor
 * to each well's centre and disagreed with the landing by half the window's
 * width, which on anything wider than the Pill is a whole column.
 */
export async function armBarSnap(grabOffset: { x: number; y: number }): Promise<void> {
  if (!isBarWindow()) return;
  snapArmed = true;
  if (overlayShown) return;
  footprintGrab = grabOffset;
  overlayShown = true;
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
      await emit(EVENTS.SNAP_WELLS_SHOW, {
        windowWidth: steady.spec.rest.width,
        windowHeight: steady.spec.rest.height,
        grabOffsetX: footprintGrab.x,
        grabOffsetY: footprintGrab.y,
      });
      return;
    }
    const logical = await logicalWindowSize(win);
    await emit(EVENTS.SNAP_WELLS_SHOW, {
      ...logical,
      grabOffsetX: grabOffset.x,
      grabOffsetY: grabOffset.y,
    });
  } catch (error) {
    console.debug("barSnap: snap-wells-show failed:", error);
  }
}

/**
 * On release after a drag, glide the bar into the nearest well and remember
 * the slot it landed in: the well decides the growth direction and the
 * anchored column from here on, and where the bar reopens next launch.
 *
 * The window's top-left snaps to the well's top-left, which is
 * distance-equivalent to centre-to-centre because every well carries the same
 * footprint, so there is one rule and no per-look data.
 */
export async function settleBarSnap(): Promise<void> {
  // Hide the drop indicator on any release path, even if the settle below
  // no-ops (drag not armed, or already consumed by another path).
  if (overlayShown) {
    overlayShown = false;
    void emit(EVENTS.SNAP_WELLS_HIDE);
  }
  if (!snapArmed || snapAnimating) return;
  snapArmed = false;
  try {
    const win = getCurrentWindow();
    const steady = getSteady(win.label);
    if (steady?.layout) {
      snapAnimating = true;
      await settleSteady(win, steady.spec, steady.layout);
      return;
    }
    const [pos, logical, monitors] = await Promise.all([
      win.outerPosition(),
      logicalWindowSize(win),
      availableMonitors(),
    ]);
    if (!monitors.length) return;
    const rects = toMonitorRects(monitors);
    const wells = computeWells(rects, { ...logical, includeCenter: true });
    const aim = (await releasedFootprintOrigin(rects)) ?? { x: pos.x, y: pos.y };
    const target = nearestWell(aim, wells);
    if (!target) return;
    snapAnimating = true;
    await animateWindowTo(win, { x: pos.x, y: pos.y }, { x: target.x, y: target.y });
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
 * the resting footprint sits on the nearest well; then, only if the new well
 * grows the other way, the frame and the drawing swap behind a brief hide
 * (`swapSteadyLayout`). The well's own position never changes in that swap,
 * so the bar lands exactly where the glide put it.
 */
async function settleSteady(win: AppWindow, spec: SteadySpec, layout: SteadyLayout) {
  const [pos, sf, monitors] = await Promise.all([
    win.outerPosition(),
    win.scaleFactor(),
    availableMonitors(),
  ]);
  if (!monitors.length) return;
  const rects = toMonitorRects(monitors);
  const offset = {
    x: Math.round(layout.anchor.x * sf),
    y: Math.round(layout.anchor.y * sf),
  };
  const anchorNow = (await releasedFootprintOrigin(rects)) ?? {
    x: pos.x + offset.x,
    y: pos.y + offset.y,
  };
  const target = nearestWell(anchorNow, steadyWells(rects, spec));
  if (!target) return;
  const next = steadyLayoutFor(target, rects, spec);
  if (!next) return;
  await animateWindowTo(
    win,
    { x: pos.x, y: pos.y },
    { x: target.x - offset.x, y: target.y - offset.y },
  );
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
 * Follow the cursor between displays: when the backend reports the cursor has
 * moved to another display, re-home the bar to the same well slot there, so it
 * is always where the user is looking.
 *
 * The slot is what survives the hop, not the coordinates: the same corner with
 * the same padding, on a display of its own size and pixel density. `paused`
 * is how a look says it is busy (the Pill pauses while its chat pane is open
 * or the agent is working); a dragging or gliding bar is never moved.
 */
export function useBarDisplayFollow(paused = false): void {
  const onCursorDisplayChange = useCallback(
    async ({ x, y }: { x: number; y: number }) => {
      if (paused || !isBarWindow()) return;
      if (barSnapBusy()) return;
      const win = getCurrentWindow();
      const slot = getDockSlot(win.label);
      if (!slot) return;
      const steady = getSteady(win.label);
      if (steady?.layout) {
        try {
          const mons = await availableMonitors();
          if (!mons.length) return;
          const rects = toMonitorRects(mons);
          const targetIdx = monitorIndexAt(rects, x, y);
          if (targetIdx < 0 || targetIdx === steady.layout.monitorIndex) return;
          const target = wellForSlot(slot, targetIdx, steadyWells(rects, steady.spec));
          if (!target) return;
          const next = steadyLayoutFor(target, rects, steady.spec);
          if (!next) return;
          await swapSteadyLayout(win.label, next, applySteadyFrame);
          setDockSlot(win.label, { fx: target.fx, fy: target.fy });
          await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(
            () => {},
          );
        } catch (error) {
          console.debug("barSnap: cursor-follow move failed:", error);
        }
        return;
      }
      try {
        const [logical, pos, mons] = await Promise.all([
          logicalWindowSize(win),
          win.outerPosition(),
          availableMonitors(),
        ]);
        if (!mons.length) return;
        const rects = toMonitorRects(mons);
        const targetIdx = monitorIndexAt(rects, x, y);
        if (targetIdx < 0) return;
        // Already on the cursor's display: nothing to do.
        if (monitorIndexAt(rects, pos.x, pos.y) === targetIdx) return;
        const wells = computeWells(rects, { ...logical, includeCenter: true });
        const target = wellForSlot(slot, targetIdx, wells);
        if (!target) return;
        await win.setPosition(new PhysicalPosition(target.x, target.y));
        setDockSlot(win.label, { fx: target.fx, fy: target.fy });
        try {
          await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y });
        } catch {
          // best effort persist
        }
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
