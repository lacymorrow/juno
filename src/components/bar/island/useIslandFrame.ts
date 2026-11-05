import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { takeBootBarPosition } from "@/lib/barBoot";
import { monitorIndexAt, setDockSlot } from "@/lib/barDock";
import { SLOT, nearestWell, wellForSlot, type Well } from "@/lib/snapWells";
import {
  dragLayout,
  getSteady,
  registerSteady,
  sameLayout,
  setSteadyFootprint,
  setSteadyLayout,
  steadyLayout,
  subscribeSteady,
  swapSteadyLayout,
  type Rect,
  type Size,
  type SteadyLayout,
} from "@/lib/steadyFrame";
import {
  applySteadyFrame,
  steadyWells,
  toMonitorRects,
  useBarDragging,
  windowOrigin,
} from "@/hooks/useBarSnapWells";
import { useEventListener } from "@/hooks/useEventListener";
import type { HoverRegion } from "./useIslandHover";
import { FLAME_HEADROOM, FLAME_HEADROOM_GROWING_UP, ISLAND_STEADY_SPEC } from "./islandModel";

/** Where a fresh install puts the Island: top centre, where an island belongs. */
const HOME_SLOT = { fx: 0.5, fy: 0 } as const;

/** Where the Island draws before its launch placement has landed (the window is still hidden). */
const FALLBACK_LAYOUT: SteadyLayout = steadyLayout(
  { x: 0, y: 0, width: 0, height: 0, fx: 0.5, fy: 0, monitorIndex: 0 },
  { position: { x: -1e6, y: -1e6 }, size: { width: 2e6, height: 2e6 }, scaleFactor: 1 },
  ISLAND_STEADY_SPEC,
);

/** The smallest rect holding every rect given. */
function boundsOf(rects: Rect[]): Rect | null {
  if (!rects.length) return null;
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  const right = Math.max(...rects.map((r) => r.x + r.width));
  const bottom = Math.max(...rects.map((r) => r.y + r.height));
  return { x, y, width: right - x, height: bottom - y };
}

export interface IslandFrame {
  layout: SteadyLayout;
  /** A drop into a well that draws differently is drawn hidden for two frames. */
  hidden: boolean;
  /** The drawn shape, from the window's top-left; the hover leave check reads it. */
  region: { current: HoverRegion | null };
  /**
   * Tell the frame what is drawn. `footprint` is the block pinned to the well
   * (shape plus its shadow room), which a drag shrinks the window to; `hit`
   * is what takes the mouse: the shape, and the roster bubble beside it when
   * there is one. The rest of the window clicks through.
   *
   * `frame` is the window this moment needs (`islandFrameFor`). `settled` says
   * the shape has reached its size: the window grows at once, but shrinks only
   * after the shape has, or the window's edge would cut the shape in half.
   */
  report: (
    footprint: { width: number; height: number },
    hit: Rect | Rect[],
    frame: Size,
    settled: boolean,
  ) => void;
  /** The window this well would have at its largest; the chat face is a share of it. */
  full: SteadyLayout;
}

/** Same size, to the pixel. */
function sameSize(a: Size | null, b: Size | null): boolean {
  if (!a || !b) return a === b;
  return a.width === b.width && a.height === b.height;
}

/**
 * The same well on screen, whatever size of window is around it. A layout is
 * this hook's own when it is on the same well as the full one: only a drop,
 * a display hop or a launch moves the well.
 */
function sameWell(a: SteadyLayout, b: SteadyLayout): boolean {
  return (
    a.origin.x + a.anchor.x === b.origin.x + b.anchor.x &&
    a.origin.y + a.anchor.y === b.origin.y + b.anchor.y &&
    a.anchorX === b.anchorX &&
    a.growUp === b.growUp &&
    a.centerY === b.centerY
  );
}

/**
 * A window of `size` on the full window's well, with the flame's room above the
 * shape, kept inside the full window. The full window is already clamped to the
 * display, so staying inside it keeps this one off the menu bar, where AppKit
 * would push it and move the well. The shift is absorbed by the anchor, as
 * `steadyLayout`'s clamp is.
 */
function frameWithin(full: SteadyLayout, size: Size): SteadyLayout {
  // Growing down, the room is only what the full window got: on the top row the
  // menu bar allows 12 points, and a taller window would just reach further down.
  const room = full.growUp ? FLAME_HEADROOM_GROWING_UP : Math.min(full.anchor.y, FLAME_HEADROOM);
  const width = Math.min(size.width, full.size.width);
  const height = Math.min(size.height + room, full.size.height);
  const placed = dragLayout(full, { width, height });
  const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
  const x = clamp(placed.origin.x, full.origin.x, full.origin.x + full.size.width - width);
  const top = full.growUp ? placed.origin.y : placed.origin.y - room;
  const y = clamp(top, full.origin.y, full.origin.y + full.size.height - height);
  return {
    ...placed,
    origin: { x, y },
    anchor: {
      ...placed.anchor,
      x: placed.anchor.x + placed.origin.x - x,
      y: placed.anchor.y + placed.origin.y - y,
    },
  };
}

/**
 * The Island's window: the steady frame (`lib/steadyFrame.ts`), the same
 * contract the Pill and the Notch keep, with one exception. Their windows are
 * a modest fixed size. The Island's largest face is a share of the display, and
 * macOS lists a window by its frame, so a window that size stops a screen-share
 * picker reaching any app behind it, however transparent it is (LAC-4832, the
 * reasoning in full above `frameAround`). So this window is only ever as big as
 * the moment needs (`islandFrameFor`).
 *
 * Crossing from one size to another is the hidden two-frame swap a drag already
 * uses (~70ms). A spoken turn crosses twice: small to a card for the answer, and
 * back. Hover, listening, transcribing and working all share the small window,
 * so they cross nothing. Launch and the tray's re-centre place the right size
 * directly. A drop or a display hop blinks twice, because the shared settle
 * grows the window to full before this shrinks it again.
 */
export function useIslandFrame(): IslandFrame {
  const label = useRef(getCurrentWindow().label).current;
  const region = useRef<HoverRegion | null>(null);
  const [placed, setPlaced] = useState(false);
  // `full` is the window whatever placed it last made (launch, a drop's settle,
  // a display hop); `asked` is the last frame this hook asked for, so one swap
  // is not queued twice while it lands.
  const frames = useRef<{ full: SteadyLayout | null; asked: SteadyLayout | null }>({
    full: null,
    asked: null,
  });
  // What the page is asking for, as state (it drives the swap) and as a ref
  // (placement reads it without waiting for a render).
  const [wanted, setWanted] = useState<Size | null>(null);
  const wantedRef = useRef<Size | null>(null);
  const [settled, setSettled] = useState(true);
  const dragging = useBarDragging();

  useLayoutEffect(() => {
    registerSteady(label, ISLAND_STEADY_SPEC);
    return () => {
      registerSteady(label, null);
      // Another look is taking the window: it gets every mouse event again.
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions: null }).catch(() => {});
    };
  }, [label]);

  const subscribe = useCallback((fn: () => void) => subscribeSteady(label, fn), [label]);
  const read = useCallback(() => getSteady(label), [label]);
  const steady = useSyncExternalStore(subscribe, read, read);
  const layout = steady?.layout ?? FALLBACK_LAYOUT;

  /**
   * Put the window in `target`'s well, in one transaction, and remember it.
   * The frame the page is asking for is the one placed: launch and the tray's
   * re-centre then never put a display-sized window on screen, so neither has
   * to blink its way down to the right size.
   */
  const settleInto = useCallback(
    async (target: Well, rects: ReturnType<typeof toMonitorRects>) => {
      const mon = rects[target.monitorIndex];
      if (!mon) return;
      const full = steadyLayout(target, mon, ISLAND_STEADY_SPEC);
      const asked = wantedRef.current;
      const next = asked ? frameWithin(full, asked) : full;
      frames.current = { full, asked: next };
      await applySteadyFrame(next);
      setSteadyLayout(label, next);
      setDockSlot(label, { fx: target.fx, fy: target.fy });
      await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(() => {});
    },
    [label],
  );

  // Launch: the remembered well (re-snapped to the nearest current one), or
  // top centre on the display the window opened on; then show. Frame first,
  // so the first thing on screen is already in its place.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const booted = takeBootBarPosition();
        const saved =
          booted !== undefined
            ? booted
            : await invoke<{ x: number; y: number } | null>(COMMANDS.BAR_GET_BAR_POSITION);
        const [pos, mons] = await Promise.all([windowOrigin(getCurrentWindow()), availableMonitors()]);
        if (cancelled || !mons.length) return;
        const rects = toMonitorRects(mons);
        const wells = steadyWells(rects, ISLAND_STEADY_SPEC);
        let target: Well | null = saved ? nearestWell(saved, wells) : null;
        if (!target) {
          const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
          target = wellForSlot(HOME_SLOT, mon, wells) ?? wells[0] ?? null;
        }
        if (cancelled || !target) return;
        await settleInto(target, rects);
      } catch (error) {
        console.debug("Island: placement failed:", error);
      } finally {
        if (!cancelled) {
          setPlaced(true);
          await invoke(COMMANDS.BAR_SHOW_BAR_WHEN_READY).catch(() => {});
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [settleInto]);

  // The tray's "Center Floating Bar": the way back from a well that is hard to reach.
  useEventListener(EVENTS.MENU_CENTER_FLOATING_BAR_REQUESTED, () => {
    void (async () => {
      try {
        const [pos, mons] = await Promise.all([windowOrigin(getCurrentWindow()), availableMonitors()]);
        if (!mons.length) return;
        const rects = toMonitorRects(mons);
        const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
        const target = wellForSlot(SLOT.center, mon, steadyWells(rects, ISLAND_STEADY_SPEC));
        if (target) await settleInto(target, rects);
      } catch (error) {
        console.debug("Island: centre failed:", error);
      }
    })();
  });

  const lastReported = useRef("");
  const report = useCallback(
    (footprint: { width: number; height: number }, hit: Rect | Rect[], frame: Size, shapeSettled: boolean) => {
      wantedRef.current = frame;
      setWanted((prev) => (sameSize(prev, frame) ? prev : frame));
      setSettled(shapeSettled);
      const regions = Array.isArray(hit) ? hit : [hit];
      // A leave is checked against everything drawn, so moving from the
      // shape onto the roster beside it is not leaving the island.
      region.current = boundsOf(regions);
      setSteadyFootprint(label, footprint);
      if (!placed) return;
      const key = regions.map((r) => `${r.x},${r.y},${r.width},${r.height}`).join(";");
      if (key === lastReported.current) return;
      lastReported.current = key;
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions }).catch((error) =>
        console.error("Island: could not set hit regions:", error),
      );
    },
    [label, placed],
  );

  // ── The window follows the moment (LAC-4832) ──
  //
  // One effect, so the two halves cannot run out of order: adopt whatever
  // placed the window as the full frame, then put the window on the frame the
  // page is asking for. Nothing happens mid-gesture: a drag has shrunk the
  // window to the shape itself and its settle grows it back, and reading that
  // drag window as `full` would leave the island stuck in it.
  const stored = steady?.layout ?? null;
  useEffect(() => {
    if (!placed || dragging) return;
    const held = frames.current;
    if (stored && !(held.full && sameWell(stored, held.full))) {
      held.full = stored;
      held.asked = null;
    }
    if (!held.full || !wanted) return;
    const want = frameWithin(held.full, wanted);
    if (held.asked ? sameLayout(held.asked, want) : stored !== null && sameLayout(stored, want)) return;
    const shrinks =
      stored !== null && want.size.width <= stored.size.width && want.size.height <= stored.size.height;
    if (shrinks && !settled) return;
    held.asked = want;
    void swapSteadyLayout(label, want, applySteadyFrame).catch((error) => {
      held.asked = null;
      console.debug("Island: frame change failed:", error);
    });
    // This runs after the paint, so the render that first draws a bigger face
    // still has the smaller window. IslandBar draws nothing for that render
    // (it compares what it is drawing against `layout.size`), because a
    // 360-wide composer clipped to a small window is a visible flash of the
    // wrong geometry.
  }, [placed, dragging, stored, wanted, settled, label]);

  const held = frames.current;
  const full = stored && held.full && sameWell(stored, held.full) ? held.full : layout;
  return { layout, hidden: steady?.hidden ?? false, region, report, full };
}
