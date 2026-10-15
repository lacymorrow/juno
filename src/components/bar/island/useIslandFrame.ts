import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { takeBootBarPosition } from "@/lib/barBoot";
import { monitorIndexAt, setDockSlot } from "@/lib/barDock";
import { SLOT, nearestWell, wellForSlot, type Well } from "@/lib/snapWells";
import {
  getSteady,
  registerSteady,
  setSteadyFootprint,
  setSteadyLayout,
  steadyLayout,
  subscribeSteady,
  type Rect,
  type SteadyLayout,
} from "@/lib/steadyFrame";
import { applySteadyFrame, steadyWells, toMonitorRects, windowOrigin } from "@/hooks/useBarSnapWells";
import { useEventListener } from "@/hooks/useEventListener";
import type { HoverRegion } from "./useIslandHover";
import { ISLAND_STEADY_SPEC } from "./islandModel";

/** Where a fresh install puts the Island: top centre, where an island belongs. */
const HOME_SLOT = { fx: 0.5, fy: 0 } as const;

/** Where the Island draws before its launch placement has landed (the window is still hidden). */
const FALLBACK_LAYOUT: SteadyLayout = steadyLayout(
  { x: 0, y: 0, width: 0, height: 0, fx: 0.5, fy: 0, monitorIndex: 0 },
  { position: { x: -1e6, y: -1e6 }, size: { width: 2e6, height: 2e6 }, scaleFactor: 1 },
  ISLAND_STEADY_SPEC,
);

export interface IslandFrame {
  layout: SteadyLayout;
  /** A drop into a well that draws differently is drawn hidden for two frames. */
  hidden: boolean;
  /** The drawn shape, from the window's top-left; the hover leave check reads it. */
  region: { current: HoverRegion | null };
  /**
   * Tell the frame what is drawn. `footprint` is the block pinned to the well
   * (shape plus its shadow room), which a drag shrinks the window to; `hit`
   * is the part that takes the mouse. The rest of the window clicks through.
   */
  report: (footprint: { width: number; height: number }, hit: Rect) => void;
}

/**
 * The Island's window: the steady frame (`lib/steadyFrame.ts`), the same
 * contract the Pill and the Notch keep. The window is sized once for the
 * largest face and placed once per well; every face change is animation
 * inside it, pinned to the well's docked edges, and the transparent rest
 * lets clicks through. Nothing here resizes the window for a state.
 */
export function useIslandFrame(): IslandFrame {
  const label = useRef(getCurrentWindow().label).current;
  const region = useRef<HoverRegion | null>(null);
  const [placed, setPlaced] = useState(false);

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

  /** Put the window in `target`'s well, in one transaction, and remember it. */
  const settleInto = useCallback(
    async (target: Well, rects: ReturnType<typeof toMonitorRects>) => {
      const mon = rects[target.monitorIndex];
      if (!mon) return;
      const next = steadyLayout(target, mon, ISLAND_STEADY_SPEC);
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
    (footprint: { width: number; height: number }, hit: Rect) => {
      region.current = hit;
      setSteadyFootprint(label, footprint);
      if (!placed) return;
      const key = `${hit.x},${hit.y},${hit.width},${hit.height}`;
      if (key === lastReported.current) return;
      lastReported.current = key;
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions: [hit] }).catch((error) =>
        console.error("Island: could not set hit regions:", error),
      );
    },
    [label, placed],
  );

  return { layout, hidden: steady?.hidden ?? false, region, report };
}
