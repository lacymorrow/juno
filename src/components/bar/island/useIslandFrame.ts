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
import { ISLAND_STEADY_SPEC } from "./islandModel";

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
   * `rest` is the window the island can rest in while this is all it draws
   * (`islandRestFrame`), or null while the drawn face needs the whole window.
   * Required, not optional: a face that forgets to decide would leave the
   * window at its display-sized frame for good, and that is the bug.
   */
  report: (
    footprint: { width: number; height: number },
    hit: Rect | Rect[],
    rest: Size | null,
  ) => void;
}

/** Same size, to the pixel. */
function sameSize(a: Size | null, b: Size | null): boolean {
  if (!a || !b) return a === b;
  return a.width === b.width && a.height === b.height;
}

/** A resting frame can never be bigger than the window it rests inside. */
function restWithin(full: SteadyLayout, rest: Size): SteadyLayout {
  return dragLayout(full, {
    width: Math.min(rest.width, full.size.width),
    height: Math.min(rest.height, full.size.height),
  });
}

/**
 * The Island's window: the steady frame (`lib/steadyFrame.ts`), the same
 * contract the Pill and the Notch keep. The window is sized for the largest
 * face and placed once per well; every face change is animation inside it,
 * pinned to the well's docked edges, and the transparent rest lets clicks
 * through.
 *
 * **One exception, and it is the whole reason this hook is longer than the
 * Pill's.** The Island's window is most of the display, because its chat face
 * is a share of the display (`islandMaxFor`). macOS lists a window by its
 * frame, so that transparent acreage is a solid Juno rectangle to any
 * screen-share picker and no app behind it can be picked (LAC-4832, and the
 * reasoning in full on `islandRestFrame`). So the window holds the full face
 * only while a full face is drawn: at rest it is the capsule
 * (`islandRestFrame`), carried there by exactly the hidden two-frame swap a
 * drag already uses to shrink this same window to the shape it is drawing.
 *
 * The boundary is resting against not-resting, never a state inside a turn:
 * hover, listening, transcribing, working, an answer and the chat face all
 * still happen in one unchanging window, which is what the steady frame is
 * for. Crossing it costs the two hidden frames `swapSteadyLayout` takes
 * (~70ms), paid when the island leaves rest and again when it returns, where
 * someone has just acted (pressed the hotkey, clicked the capsule) and never
 * during the morph that follows. Launch and the tray's re-centre place the
 * resting frame directly, so neither blinks. A drop or a display hop while
 * resting blinks twice: the shared settle (`settleSteady`) grows the window
 * back to full before this shrinks it again. Both are deliberate gestures
 * nobody makes often, so the shared settle every look uses was left alone.
 */
export function useIslandFrame(): IslandFrame {
  const label = useRef(getCurrentWindow().label).current;
  const region = useRef<HoverRegion | null>(null);
  const [placed, setPlaced] = useState(false);
  // The two frames this window moves between. `full` is whatever placed it
  // last (launch, a drop's settle, a display hop); `rest` is the shrunken one
  // this hook installed, kept so a layout arriving from the store can be told
  // apart from one of ours.
  const frames = useRef<{ full: SteadyLayout | null; rest: SteadyLayout | null }>({
    full: null,
    rest: null,
  });
  // What the page is asking for, as state (it drives the swap) and as a ref
  // (placement reads it without waiting for a render).
  const [restWanted, setRestWanted] = useState<Size | null>(null);
  const restWantedRef = useRef<Size | null>(null);
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
   * If the island is resting, the resting frame is the one placed: launch and
   * the tray's re-centre then never put a display-sized window on screen, so
   * neither has to blink its way back down to the capsule.
   */
  const settleInto = useCallback(
    async (target: Well, rects: ReturnType<typeof toMonitorRects>) => {
      const mon = rects[target.monitorIndex];
      if (!mon) return;
      const full = steadyLayout(target, mon, ISLAND_STEADY_SPEC);
      const wanted = restWantedRef.current;
      const next = wanted ? restWithin(full, wanted) : full;
      frames.current = { full, rest: wanted ? next : null };
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
    (footprint: { width: number; height: number }, hit: Rect | Rect[], rest: Size | null) => {
      restWantedRef.current = rest;
      setRestWanted((prev) => (sameSize(prev, rest) ? prev : rest));
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

  // ── The resting frame (LAC-4832) ──
  //
  // One effect, so the two halves cannot run out of order: adopt whatever
  // placed the window as the frame to rest from, then put the window on the
  // frame the page is asking for. Nothing happens mid-gesture: a drag has
  // shrunk the window to the shape itself and its settle grows it back, and
  // reading that drag window as `full` would leave the island stuck in it.
  const stored = steady?.layout ?? null;
  useEffect(() => {
    if (!placed || dragging) return;
    const held = frames.current;
    if (stored && !(held.rest && sameLayout(stored, held.rest))) {
      held.full = stored;
      held.rest = null;
    }
    if (!held.full) return;
    const want = restWanted ? restWithin(held.full, restWanted) : held.full;
    if (stored && sameLayout(stored, want)) return;
    held.rest = restWanted ? want : null;
    void swapSteadyLayout(label, want, applySteadyFrame).catch((error) =>
      console.debug("Island: resting frame failed:", error),
    );
  }, [placed, dragging, stored, restWanted, label]);

  return { layout, hidden: steady?.hidden ?? false, region, report };
}
