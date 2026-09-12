/**
 * SnapWellsOverlay — the drop-target affordance for the floating bar.
 *
 * While the bar is being dragged, dim the whole screen and cut a "hole" at each
 * snap well so the user sees where the bar will land — the way Wispr Flow /
 * superwhisper show drop targets. When the drag ends the overlay disappears.
 * The bar itself snaps to the nearest well on release (`settleIntoWell` in
 * FloatingBar); this is purely the visual affordance for it.
 *
 * Modeled on DesktopCursorOverlay: a full-screen, click-through, always-on-top
 * transparent window spanning the union of every monitor. The bar broadcasts
 * `snap-wells-show` / `snap-wells-hide` events (see FloatingBar); we own only
 * the window show/hide and the dim/hole rendering — no business logic.
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
import { computeWells, type MonitorRect, type Well } from "@/lib/snapWells";
import {
  unionLogicalBounds,
  wellToOverlayRect,
  type OverlayRect,
} from "@/lib/snapWellsOverlay";

// Corner radius of each hole / ring, in CSS px. Roughly matches the bar pill.
const HOLE_RADIUS = 14;
// Dim strength of the screen outside the wells.
const DIM = "rgba(0, 0, 0, 0.30)";
// If no hide arrives (a dropped event, a crash mid-drag), hide defensively.
const AUTO_HIDE_MS = 8000;
// How often we re-check the cursor to brighten the nearest well.
const HIGHLIGHT_POLL_MS = 80;

type ShowPayload = { windowWidth: number; windowHeight: number };

// A rendered hole: overlay-local CSS rect plus the well's physical centre (used
// to pick the nearest well to the cursor) and a stable key.
interface Hole {
  key: string;
  rect: OverlayRect;
  centerPhysX: number;
  centerPhysY: number;
}

export const SnapWellsOverlay = () => {
  // Union size of the overlay window, in CSS px (= logical span).
  const [span, setSpan] = useState<{ w: number; h: number }>({ w: 0, h: 0 });
  const [holes, setHoles] = useState<Hole[]>([]);
  const [visible, setVisible] = useState(false);
  // Index of the well nearest the cursor, brightened while dragging.
  const [highlight, setHighlight] = useState<number>(-1);

  const holesRef = useRef<Hole[]>([]);
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
  // Poll the cursor and brighten the well whose centre it's closest to, so the
  // user sees which well they'll snap to. Purely visual; safe to no-op.
  const startHighlightPoll = useCallback(() => {
    stopHighlightPoll();
    pollTimer.current = setInterval(async () => {
      try {
        const c = await cursorPosition();
        const list = holesRef.current;
        if (!list.length) return;
        let bestIdx = -1;
        let bestD = Infinity;
        for (let i = 0; i < list.length; i++) {
          const dx = c.x - list[i].centerPhysX;
          const dy = c.y - list[i].centerPhysY;
          const d = dx * dx + dy * dy;
          if (d < bestD) {
            bestD = d;
            bestIdx = i;
          }
        }
        // Only re-render when the nearest well actually changes.
        setHighlight((prev) => (prev === bestIdx ? prev : bestIdx));
      } catch {
        // cursorPosition can transiently fail; ignore and try next tick.
      }
    }, HIGHLIGHT_POLL_MS);
  }, [stopHighlightPoll]);

  // ── Show: size+position the overlay over all monitors, then compute holes ──
  const show = useCallback(
    async ({ windowWidth, windowHeight }: ShowPayload) => {
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
        // the same includeCenter (FloatingBar's settleIntoWell enables the
        // centre well, so this must too, or the indicator would miss a hole).
        const wells: Well[] = computeWells(monitorRects, {
          windowWidth,
          windowHeight,
          includeCenter: true,
        });

        const nextHoles: Hole[] = wells.map((w, i) => {
          const rect = wellToOverlayRect(
            w,
            monitorRects,
            { x: bounds.originX, y: bounds.originY },
            { width: windowWidth, height: windowHeight },
          );
          return {
            key: `${w.monitorIndex}-${w.row}-${w.col}-${i}`,
            rect,
            centerPhysX: w.x + windowWidth / 2,
            centerPhysY: w.y + windowHeight / 2,
          };
        });

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

  useEventListener<ShowPayload>("snap-wells-show", (payload) => {
    void show(payload);
  });

  useEventListener("snap-wells-hide", () => {
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
