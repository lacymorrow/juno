/**
 * ListeningGlow — the "Juno is listening" screen-edge indicator (route
 * `/listening-overlay`, its own transparent, click-through, always-on-top
 * window declared in tauri.conf.json).
 *
 * Siri's model: the moment the wake phrase lands, acknowledge (a chime, played
 * by the backend) and show a soft glow around the screen edge while the request
 * is captured; drop it the instant the request is in hand. The backend owns the
 * listening state and already emits it, so this window only listens and renders
 * (the frontend-is-display-only rule): it never decides when Juno is listening.
 *
 * Visual: a single flat system-blue inset glow, breathing gently. No gradient,
 * no purple, no rainbow sweep, per Juno's design rules; the accent is the one
 * macOS system blue. The breathing is disabled under prefers-reduced-motion.
 *
 * Window management mirrors SnapWellsOverlay: size + position over the union of
 * all monitors in logical px, mark the window click-through so it never eats a
 * click, then show; fade out before hiding so the glow doesn't just vanish.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window";

import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";
import { unionLogicalBounds } from "@/lib/snapWellsOverlay";
import type { MonitorRect } from "@/lib/snapWells";

/** How long the fade-out runs before the window is actually hidden (ms). */
const FADE_OUT_MS = 260;

/** The one accent: macOS system blue. */
const ACCENT = "10, 132, 255";

export const ListeningGlow = () => {
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const show = useCallback(async () => {
    if (hideTimer.current) {
      clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    try {
      const win = getCurrentWindow();

      // Union bounding box across every monitor, in logical px, so the glow
      // covers the whole desktop (mirrors SnapWellsOverlay / DesktopCursorOverlay).
      let monitorRects: MonitorRect[] = [];
      try {
        const monitors = await availableMonitors();
        monitorRects = monitors.map((m) => ({
          position: { x: m.position.x, y: m.position.y },
          size: { width: m.size.width, height: m.size.height },
          scaleFactor: m.scaleFactor,
        }));
      } catch (err) {
        console.debug("[ListeningGlow] availableMonitors failed:", err);
      }

      const bounds = unionLogicalBounds(monitorRects);
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
        // Never intercept a click: the glow is pure affordance.
        win.setIgnoreCursorEvents(true),
      ]);
      await win.show();
      setVisible(true);
    } catch (err) {
      console.debug("[ListeningGlow] show failed:", err);
    }
  }, []);

  const hide = useCallback(() => {
    setVisible(false);
    if (hideTimer.current) clearTimeout(hideTimer.current);
    // Let the opacity transition finish, then hide the OS window so it stops
    // compositing over everything.
    hideTimer.current = setTimeout(async () => {
      try {
        await getCurrentWindow().hide();
      } catch (err) {
        console.debug("[ListeningGlow] hide failed:", err);
      }
    }, FADE_OUT_MS);
  }, []);

  // Wake phrase landed -> Juno is listening to the request: glow on.
  useEventListener(EVENTS.ALWAYS_LISTENING_WAKE_WORD_DETECTED, () => {
    void show();
  });

  // The request is in hand, or listening ended: glow off.
  useEventListener(EVENTS.ALWAYS_LISTENING_COMMAND_PROCESSED, () => hide());
  useEventListener(EVENTS.ALWAYS_LISTENING_STOPPED_BY_COMMAND, () => hide());
  useEventListener(EVENTS.ALWAYS_LISTENING_DEACTIVATED, () => hide());
  // Always-listening turned off entirely.
  useEventListener<boolean>(EVENTS.ALWAYS_LISTENING_MODE_CHANGED, (active) => {
    if (active === false) hide();
  });

  useEffect(() => {
    return () => {
      if (hideTimer.current) clearTimeout(hideTimer.current);
    };
  }, []);

  return (
    <div
      aria-hidden
      style={{
        position: "fixed",
        inset: 0,
        pointerEvents: "none",
        opacity: visible ? 1 : 0,
        transition: `opacity ${FADE_OUT_MS}ms ease-out`,
      }}
    >
      <style>{`
        @keyframes juno-listening-breathe {
          0%, 100% { opacity: 0.55; }
          50%      { opacity: 1; }
        }
        .juno-listening-edge {
          position: absolute;
          inset: 0;
          /* One flat system-blue inset glow. Two stacked inset shadows: a wide
             soft wash and a tighter inner ring, so the edge reads without a
             hard line. */
          box-shadow:
            inset 0 0 140px 24px rgba(${ACCENT}, 0.30),
            inset 0 0 44px 2px  rgba(${ACCENT}, 0.45);
          animation: juno-listening-breathe 2.6s ease-in-out infinite;
        }
        @media (prefers-reduced-motion: reduce) {
          .juno-listening-edge { animation: none; opacity: 0.8; }
        }
      `}</style>
      <div className="juno-listening-edge" />
    </div>
  );
};

export default ListeningGlow;
