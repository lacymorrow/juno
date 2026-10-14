import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { takeBootBarPosition } from "@/lib/barBoot";
import { monitorIndexAt, setDockSlot } from "@/lib/barDock";
import {
  FALLBACK_MENU_BAR,
  curlCornerFor,
  menuBarHeight,
  notchEdgeFor,
  notchFormFor,
  type CurlCorner,
  type NotchEdge,
  type NotchForm,
} from "@/lib/notchWells";
import { nearestWell, wellForSlot, type Well } from "@/lib/snapWells";
import {
  contentPlacement,
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
import type { HoverRegion } from "../island/useIslandHover";
import { NOTCH_STEADY_SPEC } from "./notchModel";

/** The notch's own slot: top centre. Where a fresh install and the tray's "Center" put it. */
const NOTCH_SLOT = { fx: 0.5, fy: 0 } as const;

/** Where the Notch draws before its launch placement has landed (the window is still hidden). */
const FALLBACK_LAYOUT: SteadyLayout = steadyLayout(
  { x: 0, y: 0, width: 0, height: 0, fx: 0.5, fy: 0, monitorIndex: 0 },
  { position: { x: -1e6, y: -1e6 }, size: { width: 2e6, height: 2e6 }, scaleFactor: 1 },
  NOTCH_STEADY_SPEC,
);

export interface NotchFrame {
  layout: SteadyLayout;
  /** A drop into a well that draws differently is drawn hidden for two frames. */
  hidden: boolean;
  form: NotchForm;
  corner: CurlCorner;
  /** The edge a notch grows out of. */
  edge: NotchEdge;
  /** The menu bar strip on the well's display, in points. */
  strip: number;
  /** CSS for the block pinned to the well's docked edges. */
  placement: ReturnType<typeof contentPlacement>;
  /** The drawn shape, from the window's top-left; the hover leave check reads it. */
  region: { current: HoverRegion | null };
  /** Tell the frame what is drawn: the drag carries it and the hit test lets the rest through. */
  report: (content: Rect) => void;
}

/**
 * The Notch's window: the steady frame, in the Notch's flush edge wells.
 *
 * Same contract as the Pill (`lib/steadyFrame.ts`): the window is sized once
 * for the largest shape and placed once per well; every state is a CSS change
 * inside it, pinned to the well's docked edges, and the transparent rest lets
 * clicks through. On top of that the Notch asks Rust to lift the bar over the
 * menu bar while it rests (`set_bar_over_menu_bar`), which is what lets it sit
 * flush with the top of the display.
 *
 * `enabled` is false for the Island, which shares the component and sizes its
 * own window; then this does nothing at all.
 */
export function useNotchFrame(enabled: boolean): NotchFrame {
  const label = useRef(getCurrentWindow().label).current;
  const region = useRef<HoverRegion | null>(null);
  const [placed, setPlaced] = useState(false);
  const [strip, setStrip] = useState(FALLBACK_MENU_BAR);

  useLayoutEffect(() => {
    if (!enabled) return;
    registerSteady(label, NOTCH_STEADY_SPEC);
    return () => {
      registerSteady(label, null);
      // Another look is taking the window: every mouse event again, and back
      // under the menu bar.
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions: null }).catch(() => {});
      void invoke(COMMANDS.BAR_SET_BAR_OVER_MENU_BAR, { over: false }).catch(() => {});
    };
  }, [enabled, label]);

  const subscribe = useCallback((fn: () => void) => subscribeSteady(label, fn), [label]);
  const read = useCallback(() => getSteady(label), [label]);
  const steady = useSyncExternalStore(subscribe, read, read);
  const layout = (enabled && steady?.layout) || FALLBACK_LAYOUT;

  /** Put the window in `target`'s well, in one transaction, and remember it. */
  const settleInto = useCallback(
    async (target: Well, rects: ReturnType<typeof toMonitorRects>) => {
      const mon = rects[target.monitorIndex];
      if (!mon) return;
      const next = steadyLayout(target, mon, NOTCH_STEADY_SPEC);
      await applySteadyFrame(next);
      setSteadyLayout(label, next);
      setDockSlot(label, { fx: target.fx, fy: target.fy });
      await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(() => {});
    },
    [label],
  );

  // Launch: over the menu bar first, so the frame that follows is not pushed
  // under it; then the remembered well (re-snapped to the nearest current
  // one), or the notch on the display the window opened on; then show.
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    void (async () => {
      try {
        await invoke(COMMANDS.BAR_SET_BAR_OVER_MENU_BAR, { over: true });
        const booted = takeBootBarPosition();
        const saved =
          booted !== undefined
            ? booted
            : await invoke<{ x: number; y: number } | null>(COMMANDS.BAR_GET_BAR_POSITION);
        const [pos, mons] = await Promise.all([windowOrigin(getCurrentWindow()), availableMonitors()]);
        if (cancelled || !mons.length) return;
        const rects = toMonitorRects(mons);
        const wells = steadyWells(rects, NOTCH_STEADY_SPEC);
        let target: Well | null = saved ? nearestWell(saved, wells) : null;
        if (!target) {
          const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
          target = wellForSlot(NOTCH_SLOT, mon, wells) ?? wells[0] ?? null;
        }
        if (cancelled || !target) return;
        await settleInto(target, rects);
      } catch (error) {
        console.debug("Notch: placement failed:", error);
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
  }, [enabled, settleInto]);

  // The tray's "Center Floating Bar": for the Notch, home is the notch.
  useEventListener(EVENTS.MENU_CENTER_FLOATING_BAR_REQUESTED, () => {
    if (!enabled) return;
    void (async () => {
      try {
        const [pos, mons] = await Promise.all([windowOrigin(getCurrentWindow()), availableMonitors()]);
        if (!mons.length) return;
        const rects = toMonitorRects(mons);
        const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
        const target = wellForSlot(NOTCH_SLOT, mon, steadyWells(rects, NOTCH_STEADY_SPEC));
        if (target) await settleInto(target, rects);
      } catch (error) {
        console.debug("Notch: return to the notch failed:", error);
      }
    })();
  });

  // The menu bar on the display the well is on: 37 or 38 points under a
  // notch, 24 elsewhere. Read again whenever the well changes display.
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    availableMonitors()
      .then((mons) => {
        const mon = toMonitorRects(mons)[layout.monitorIndex];
        if (!cancelled && mon) setStrip(menuBarHeight(mon));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [enabled, layout.monitorIndex]);

  const lastReported = useRef<string>("");
  const report = useCallback(
    (content: Rect) => {
      if (!enabled) return;
      region.current = content;
      setSteadyFootprint(label, { width: content.width, height: content.height });
      if (!placed) return;
      const key = `${content.x},${content.y},${content.width},${content.height}`;
      if (key === lastReported.current) return;
      lastReported.current = key;
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions: [content] }).catch((error) =>
        console.error("Notch: could not set hit regions:", error),
      );
    },
    [enabled, label, placed],
  );

  return {
    layout,
    hidden: steady?.hidden ?? false,
    form: notchFormFor(layout.slot),
    corner: curlCornerFor(layout.slot),
    edge: notchEdgeFor(layout.slot),
    strip,
    placement: contentPlacement(layout),
    region,
    report,
  };
}
