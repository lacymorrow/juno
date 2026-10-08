/**
 * SnapWellsOverlay: the drop-target affordance for the bar, whichever look
 * is in it.
 *
 * While the bar is being dragged, dim the whole screen and cut a "hole" at each
 * snap well so the user sees where the bar will land — the way Wispr Flow /
 * superwhisper show drop targets. When the drag ends the overlay disappears.
 * The bar itself snaps to the nearest well on release (`settleBarSnap` in
 * `hooks/useBarSnapWells`); this is purely the visual affordance for it.
 *
 * One click-through, always-on-top transparent window PER DISPLAY, each
 * covering exactly its own display and drawing only its own wells. A single
 * window spanning every monitor cannot work with "Displays have separate
 * Spaces" (the macOS default): a window is drawn on one display only, so the
 * wells on every other display were missing. The first overlay is declared in
 * tauri.conf.json; Rust builds `snap-wells-overlay-N` for display N on the
 * first drag that needs it (`ensure_snap_wells_overlays`), and the window
 * reads its display from its own label. The drag gesture broadcasts
 * `snap-wells-show` / `snap-wells-hide`; every overlay hears them.
 *
 * Everything is in global desktop points (`src/lib/desktopPoints.ts`), the
 * one space that is the same on every display.
 *
 * The brightened ring is predicted with the SAME comparison the settle makes:
 * the window's top-left against each well's top-left, with that top-left
 * worked out from the cursor and the grab offset carried in the show payload.
 * It used to compare the bare cursor against each well's CENTRE, which agrees
 * with the landing only while the window is small. On a wide strip grabbed
 * near one end the error is half the window's width, easily a whole column, so
 * the ring brightened over one well and the bar landed in another.
 *
 * The 2560x1600 @ 0,0 geometry in tauri.conf.json is only a startup fallback
 * (JSON cannot carry comments): this component sizes and places the window
 * over its display before every show().
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
  availableMonitors,
  cursorPosition,
} from "@tauri-apps/api/window";
import { useEventListener } from "@/hooks/useEventListener";
import {
  computeWells,
  nearestWell,
  type MonitorRect,
  type Well,
} from "@/lib/snapWells";
import { distinctWells, predictedWindowOrigin } from "@/lib/barDock";
import { cursorInPoints, monitorsInPoints, type TauriMonitorLike } from "@/lib/desktopPoints";
import {
  overlayDisplayIndex,
  wellToOverlayRect,
  wellsForDisplay,
  type OverlayRect,
} from "@/lib/snapWellsOverlay";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

// Corner radius of each hole / ring, in CSS px. Roughly matches the bar pill.
const HOLE_RADIUS = 14;
// Dim strength of the screen outside the wells.
const DIM = "rgba(0, 0, 0, 0.30)";
// If no hide arrives (a dropped event, a crash mid-drag), hide defensively.
// It is re-armed for as long as the button is held, so a long drag keeps its
// wells however long it takes; it only fires once nobody is holding anything
// (or where the button cannot be read).
export const AUTO_HIDE_MS = 8000;
// How often the overlay asks whether the drag's button is still held.
export const HELD_CHECK_MS = 500;
// How often we re-check the cursor to brighten the nearest well.
const HIGHLIGHT_POLL_MS = 80;

/**
 * The bar's footprint in points, plus where inside it the drag was grabbed,
 * also in points.
 * The grab offset is what lets the highlight predict the window's top-left
 * from the cursor; a payload without it is treated as grabbed at the window's
 * top-left, which is the old behaviour and close enough for a small window.
 */
type ShowPayload = {
  windowWidth: number;
  windowHeight: number;
  grabOffsetX?: number;
  grabOffsetY?: number;
};

// A rendered hole: overlay-local CSS rect, the well it draws (global points,
// for the nearest-well highlight) and a stable key.
interface Hole {
  key: string;
  rect: OverlayRect;
  well: Well;
}

export const SnapWellsOverlay = () => {
  // Which display this overlay covers, from its window label.
  const displayIndex = useRef(-1);
  if (displayIndex.current < 0) {
    try {
      displayIndex.current = Math.max(0, overlayDisplayIndex(getCurrentWindow().label));
    } catch {
      displayIndex.current = 0;
    }
  }
  // Size of the overlay window, in CSS px (= its display, in points).
  const [span, setSpan] = useState<{ w: number; h: number }>({ w: 0, h: 0 });
  const [holes, setHoles] = useState<Hole[]>([]);
  const [visible, setVisible] = useState(false);
  // Index of the well nearest the cursor, brightened while dragging.
  const [highlight, setHighlight] = useState<number>(-1);

  const holesRef = useRef<Hole[]>([]);
  // The live monitor set and this drag's grab offset: together they turn a
  // cursor position into the window's predicted top-left. Every display's
  // wells take part in choosing the nearest; this overlay draws only its own.
  const monitorsRef = useRef<TauriMonitorLike[]>([]);
  const allWellsRef = useRef<Well[]>([]);
  const grabOffsetRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const autoHideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const heldTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopHighlightPoll = useCallback(() => {
    if (pollTimer.current) {
      clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
    if (heldTimer.current) {
      clearInterval(heldTimer.current);
      heldTimer.current = null;
    }
  }, []);

  const hide = useCallback(async () => {
    if (autoHideTimer.current) {
      clearTimeout(autoHideTimer.current);
      autoHideTimer.current = null;
    }
    stopHighlightPoll();
    setVisible(false);
    setHighlight(-1);
    try {
      await getCurrentWindow().hide();
    } catch (err) {
      console.debug("[SnapWells] hide failed:", err);
    }
  }, [stopHighlightPoll]);

  // ── Nearest-well highlight ────────────────────────────────────────────────
  // Poll the cursor, work out where the dragged window's top-left must be, and
  // brighten the well that top-left is nearest: the exact comparison the
  // settle makes, so the preview cannot disagree with the landing. Purely
  // visual; safe to no-op.
  const startHighlightPoll = useCallback(() => {
    stopHighlightPoll();
    pollTimer.current = setInterval(async () => {
      try {
        const c = cursorInPoints(await cursorPosition(), monitorsRef.current);
        const list = holesRef.current;
        if (!list.length) return;
        const origin = predictedWindowOrigin(c, grabOffsetRef.current);
        const target = nearestWell(origin, allWellsRef.current);
        const bestIdx = target ? list.findIndex((h) => h.well === target) : -1;
        // Only re-render when the nearest well actually changes.
        setHighlight((prev) => (prev === bestIdx ? prev : bestIdx));
      } catch {
        // cursorPosition can transiently fail; ignore and try next tick.
      }
    }, HIGHLIGHT_POLL_MS);
  }, [stopHighlightPoll]);

  // ── Show: size+position the overlay over all monitors, then compute holes ──
  const show = useCallback(
    async ({ windowWidth, windowHeight, grabOffsetX, grabOffsetY }: ShowPayload) => {
      try {
        const win = getCurrentWindow();

        let monitors: TauriMonitorLike[] = [];
        try {
          monitors = await availableMonitors();
        } catch (err) {
          console.debug("[SnapWells] availableMonitors failed:", err);
        }
        const monitorRects: MonitorRect[] = monitorsInPoints(monitors);
        const display = monitorRects[displayIndex.current];
        if (!display) {
          // A display that has gone away since this overlay was built.
          await hide();
          return;
        }

        await Promise.all([
          win.setSize(new LogicalSize(display.size.width, display.size.height)),
          win.setPosition(new LogicalPosition(display.position.x, display.position.y)),
          // Must NOT intercept the in-flight drag.
          win.setIgnoreCursorEvents(true),
        ]);

        // Same wells the bar snaps to: same computeWells, same bar size, and
        // the same includeCenter (`settleBarSnap` enables the centre well, so
        // this must too, or the indicator would miss a hole). Collapsed to one
        // well per real landing spot: a look wider than the inset area has its
        // columns converge on the same x, and rings stacked on one rectangle
        // only read as a single brighter ring.
        const wells: Well[] = distinctWells(
          computeWells(monitorRects, {
            windowWidth,
            windowHeight,
            includeCenter: true,
          }),
        );

        const nextHoles: Hole[] = wellsForDisplay(wells, displayIndex.current).map((w, i) => ({
          key: `${w.monitorIndex}-${w.fy}-${w.fx}-${i}`,
          rect: wellToOverlayRect(w, display),
          well: w,
        }));
        const spanW = display.size.width;
        const spanH = display.size.height;
        allWellsRef.current = wells;

        monitorsRef.current = monitors;
        grabOffsetRef.current = { x: grabOffsetX ?? 0, y: grabOffsetY ?? 0 };
        holesRef.current = nextHoles;
        setSpan({ w: spanW, h: spanH });
        setHoles(nextHoles);
        setHighlight(-1);
        setVisible(true);

        await win.show();

        // Showing put this overlay in front of everything at its level, the
        // bar included; put the bar back on top so the dim never covers the
        // pill being dragged.
        void invoke(COMMANDS.BAR_ORDER_ABOVE_SNAP_WELLS).catch((err) =>
          console.debug("[SnapWells] could not keep the bar above:", err),
        );

        // (Re)arm the defensive auto-hide and the nearest-well poll.
        const armAutoHide = () => {
          if (autoHideTimer.current) clearTimeout(autoHideTimer.current);
          autoHideTimer.current = setTimeout(() => void hide(), AUTO_HIDE_MS);
        };
        armAutoHide();
        startHighlightPoll();
        // A drag held past the auto-hide keeps its wells: while the button is
        // down the timer is pushed back. Once it is up the drag is over,
        // whether or not the bar's hide reached us.
        heldTimer.current = setInterval(() => {
          void invoke<boolean>(COMMANDS.BAR_POINTER_HELD)
            .then((held) => {
              if (held === true) armAutoHide();
              else if (held === false) void hide();
            })
            .catch(() => {
              if (heldTimer.current) clearInterval(heldTimer.current);
              heldTimer.current = null;
            });
        }, HELD_CHECK_MS);
      } catch (err) {
        console.error("[SnapWells] show failed:", err);
      }
    },
    [hide, startHighlightPoll],
  );

  useEventListener<ShowPayload>(EVENTS.SNAP_WELLS_SHOW, (payload) => {
    void show(payload);
  });

  useEventListener(EVENTS.SNAP_WELLS_HIDE, () => {
    void hide();
  });

  // Built mid-drag (a display's first drag builds its overlay), this window
  // missed the show. Say it is listening; the bar answers with the payload.
  useEffect(() => {
    const t = setTimeout(() => void emit(EVENTS.SNAP_WELLS_READY).catch(() => {}), 50);
    return () => clearTimeout(t);
  }, []);

  // Cleanup on unmount.
  useEffect(() => {
    return () => {
      if (autoHideTimer.current) clearTimeout(autoHideTimer.current);
      stopHighlightPoll();
    };
  }, [stopHighlightPoll]);

  // A unique mask id keeps multiple overlay instances (dev hot-reload) from
  // colliding on the same SVG mask.
  const maskId = "snap-wells-mask";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        pointerEvents: "none",
        overflow: "hidden",
        background: "transparent",
        opacity: visible ? 1 : 0,
        transition: "opacity 0.12s ease",
      }}
    >
      {visible && span.w > 0 && (
        <svg
          width={span.w}
          height={span.h}
          viewBox={`0 0 ${span.w} ${span.h}`}
          style={{ position: "absolute", top: 0, left: 0, pointerEvents: "none" }}
          aria-hidden="true"
        >
          <defs>
            <mask id={maskId}>
              {/* White = dimmed, black = clear cut-out. */}
              <rect x={0} y={0} width={span.w} height={span.h} fill="white" />
              {holes.map((h) => (
                <rect
                  key={`hole-${h.key}`}
                  x={h.rect.x}
                  y={h.rect.y}
                  width={h.rect.w}
                  height={h.rect.h}
                  rx={HOLE_RADIUS}
                  ry={HOLE_RADIUS}
                  fill="black"
                />
              ))}
            </mask>
          </defs>

          {/* Dim layer with the wells punched out. */}
          <rect
            x={0}
            y={0}
            width={span.w}
            height={span.h}
            fill={DIM}
            mask={`url(#${maskId})`}
          />

          {/* Subtle ring around each hole; the nearest well is brightened. */}
          {holes.map((h, i) => {
            const active = i === highlight;
            return (
              <rect
                key={`ring-${h.key}`}
                x={h.rect.x}
                y={h.rect.y}
                width={h.rect.w}
                height={h.rect.h}
                rx={HOLE_RADIUS}
                ry={HOLE_RADIUS}
                fill="none"
                stroke={active ? "rgba(255,255,255,0.9)" : "rgba(255,255,255,0.4)"}
                strokeWidth={active ? 1.5 : 1}
                style={{ transition: "stroke 0.12s ease, stroke-width 0.12s ease" }}
              />
            );
          })}
        </svg>
      )}
    </div>
  );
};

export default SnapWellsOverlay;
