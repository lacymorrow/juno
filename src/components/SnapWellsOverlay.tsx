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
 * Modeled on DesktopCursorOverlay: a full-screen, click-through, always-on-top
 * transparent window spanning the union of every monitor. The drag gesture
 * broadcasts `snap-wells-show` / `snap-wells-hide`; we own only the window
 * show/hide and the dim/hole rendering, and no business logic.
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
 * (JSON cannot carry comments): this component resizes/positions the window
 * to span the live monitor set before every show().
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
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
import {
  unionLogicalBounds,
  wellToOverlayRect,
  type OverlayRect,
} from "@/lib/snapWellsOverlay";
import { EVENTS } from "@/lib/constants.generated";

// Corner radius of each hole / ring, in CSS px. Roughly matches the bar pill.
const HOLE_RADIUS = 14;
// Dim strength of the screen outside the wells.
const DIM = "rgba(0, 0, 0, 0.30)";
// If no hide arrives (a dropped event, a crash mid-drag), hide defensively.
const AUTO_HIDE_MS = 8000;
// How often we re-check the cursor to brighten the nearest well.
const HIGHLIGHT_POLL_MS = 80;

/**
 * The bar's size in logical pixels (each well scales it for its own display),
 * plus where inside the window the drag was grabbed, also in logical pixels.
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

// A rendered hole: overlay-local CSS rect, the well it draws (physical px, for
// the nearest-well highlight) and a stable key.
interface Hole {
  key: string;
  rect: OverlayRect;
  well: Well;
}

export const SnapWellsOverlay = () => {
  // Union size of the overlay window, in CSS px (= logical span).
  const [span, setSpan] = useState<{ w: number; h: number }>({ w: 0, h: 0 });
  const [holes, setHoles] = useState<Hole[]>([]);
  const [visible, setVisible] = useState(false);
  // Index of the well nearest the cursor, brightened while dragging.
  const [highlight, setHighlight] = useState<number>(-1);

  const holesRef = useRef<Hole[]>([]);
  // The live monitor set and this drag's grab offset: together they turn a
  // cursor position into the window's predicted top-left.
  const monitorsRef = useRef<MonitorRect[]>([]);
  const grabOffsetRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const autoHideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopHighlightPoll = useCallback(() => {
    if (pollTimer.current) {
      clearInterval(pollTimer.current);
      pollTimer.current = null;
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
        const c = await cursorPosition();
        const list = holesRef.current;
        if (!list.length) return;
        const origin = predictedWindowOrigin(
          c,
          grabOffsetRef.current,
          monitorsRef.current,
        );
        const target = nearestWell(
          origin,
          list.map((h) => h.well),
        );
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

        // Union bounding box across every monitor, in logical px — mirrors
        // DesktopCursorOverlay so the overlay covers the same area and holes
        // line up across displays.
        let monitorRects: MonitorRect[] = [];
        try {
          const monitors = await availableMonitors();
          monitorRects = monitors.map((m) => ({
            position: { x: m.position.x, y: m.position.y },
            size: { width: m.size.width, height: m.size.height },
            scaleFactor: m.scaleFactor,
          }));
        } catch (err) {
          console.debug("[SnapWells] availableMonitors failed:", err);
        }

        const bounds = unionLogicalBounds(monitorRects);
        // Fall back to the primary display if the monitor API gave us nothing.
        const spanW = bounds.width || window.screen.width;
        const spanH = bounds.height || window.screen.height;

        await Promise.all([
          win.setSize(new LogicalSize(spanW, spanH)),
          win.setPosition(
            new PhysicalPosition(
              Math.round(bounds.originX),
              Math.round(bounds.originY),
            ),
          ),
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

        const nextHoles: Hole[] = wells.map((w, i) => {
          const rect = wellToOverlayRect(
            w,
            monitorRects,
            { x: bounds.originX, y: bounds.originY },
            { width: w.width, height: w.height },
          );
          return {
            key: `${w.monitorIndex}-${w.fy}-${w.fx}-${i}`,
            rect,
            well: w,
          };
        });

        monitorsRef.current = monitorRects;
        grabOffsetRef.current = { x: grabOffsetX ?? 0, y: grabOffsetY ?? 0 };
        holesRef.current = nextHoles;
        setSpan({ w: spanW, h: spanH });
        setHoles(nextHoles);
        setHighlight(-1);
        setVisible(true);

        await win.show();

        // (Re)arm the defensive auto-hide and the nearest-well poll.
        if (autoHideTimer.current) clearTimeout(autoHideTimer.current);
        autoHideTimer.current = setTimeout(() => void hide(), AUTO_HIDE_MS);
        startHighlightPoll();
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
