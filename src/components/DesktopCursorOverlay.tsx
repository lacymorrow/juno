import { useEffect, useRef } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import { useEventListener } from "@/hooks/useEventListener";

type CursorState = "idle" | "moving" | "clicking" | "thinking";

const TRAIL_COUNT = 5;
// Matches the cx/cy of the hot-spot circle in JunoCursorShape (SVG viewBox coords).
// Update this if the SVG geometry changes — it controls transform-origin and the
// translate offset so the arrow tip sits exactly at the reported screen coordinate.
const HOT_SPOT = 5;
const CURSOR_FADE_DELAY_MS = 1500;
const CLICK_ANIM_DURATION_MS = 700;

// ─── CSS Animations ───────────────────────────────────────────────────────────
// All cursor animations live here to keep them out of the component render path.
const CURSOR_CSS = `
  .juno-cursor {
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
    will-change: transform, opacity;
    transition: opacity 0.35s ease;
  }

  /* Idle: soft breathing pulse around the hot-spot */
  .juno-cursor--idle svg {
    animation: juno-breathe 3s ease-in-out infinite;
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
  }

  /* Thinking: Codex-style gentle lateral wobble */
  .juno-cursor--thinking svg {
    animation: juno-wobble 0.55s ease-in-out infinite;
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
  }

  /* Clicking: quick snap-back recoil on the tip */
  .juno-cursor--clicking svg {
    animation: juno-recoil 0.22s ease-out;
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
  }

  @keyframes juno-breathe {
    0%, 100% { opacity: 0.72; transform: scale(1); }
    50%       { opacity: 0.96; transform: scale(1.05); }
  }

  @keyframes juno-wobble {
    0%, 100% { transform: rotate(0deg)   translateX(0px); }
    20%      { transform: rotate(-5deg)  translateX(-1.5px); }
    40%      { transform: rotate(4deg)   translateX(1.5px); }
    60%      { transform: rotate(-3deg)  translateX(-1px); }
    80%      { transform: rotate(2.5deg) translateX(0.8px); }
  }

  @keyframes juno-recoil {
    0%   { transform: scale(1)    rotate(0deg); }
    30%  { transform: scale(0.83) rotate(-7deg); }
    100% { transform: scale(1)    rotate(0deg); }
  }

  /* Click ripple: emanates from click position, uses left/top for placement */
  @keyframes juno-ripple {
    0%   { transform: scale(0.2); opacity: 1; }
    100% { transform: scale(3.2); opacity: 0; }
  }
  .juno-ripple-active {
    animation: juno-ripple 0.7s ease-out forwards;
  }
`;

// ─── Juno Cursor SVG ──────────────────────────────────────────────────────────
// Distinctive arrow cursor: hot-spot at (5, 5), gradient body, glowing tip.
// The arrow points upper-left so the very tip is at the registered hot-spot.
const JunoCursorShape = () => (
  <svg
    width="36"
    height="44"
    viewBox="0 0 36 44"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    style={{ display: "block" }}
    aria-hidden="true"
  >
    <defs>
      <filter id="juno-glow" x="-65%" y="-55%" width="230%" height="210%">
        <feGaussianBlur stdDeviation="2.5" result="blur" />
        <feMerge>
          <feMergeNode in="blur" />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
      {/* Purple-to-dark gradient body */}
      <linearGradient
        id="juno-body"
        x1="5"
        y1="5"
        x2="30"
        y2="42"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stopColor="rgba(139, 92, 246, 0.95)" />
        <stop offset="100%" stopColor="rgba(12, 8, 55, 0.92)" />
      </linearGradient>
    </defs>

    {/* Soft drop shadow (offset copy, no stroke) */}
    <path
      d="M5 5 L5 34 L13 25 L17.5 37 L22 35 L17.5 23 L30 23 Z"
      fill="rgba(0,0,0,0.32)"
      transform="translate(1.5, 1.5)"
    />

    {/* Main arrow body */}
    <path
      d="M5 5 L5 34 L13 25 L17.5 37 L22 35 L17.5 23 L30 23 Z"
      fill="url(#juno-body)"
      stroke="rgba(167, 139, 250, 0.88)"
      strokeWidth="1.5"
      strokeLinejoin="round"
      filter="url(#juno-glow)"
    />

    {/* Hot-spot outer glow ring */}
    <circle
      cx="5"
      cy="5"
      r="4.5"
      fill="rgba(139, 92, 246, 0.45)"
      filter="url(#juno-glow)"
    />
    {/* Hot-spot bright core */}
    <circle cx="5" cy="5" r="2" fill="white" />
  </svg>
);

// ─── Component ────────────────────────────────────────────────────────────────

export const DesktopCursorOverlay = () => {
  // DOM refs — manipulated directly to avoid React state re-renders at 60 fps
  const cursorRef = useRef<HTMLDivElement>(null);
  const trailRefs = useRef<(HTMLDivElement | null)[]>([]);
  const rippleRefs = useRef<(HTMLDivElement | null)[]>([]);

  // Animation state — refs only, no React state
  const trailBuffer = useRef<{ x: number; y: number }[]>([]);
  const stateRef = useRef<CursorState>("idle");
  const nextRippleIdx = useRef(0);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Imperative helpers ───────────────────────────────────────────────────

  const applyState = (state: CursorState) => {
    stateRef.current = state;
    if (cursorRef.current) {
      cursorRef.current.className = `juno-cursor juno-cursor--${state}`;
    }
  };

  const moveCursorTo = (x: number, y: number) => {
    if (!cursorRef.current) return;
    // Offset by HOT_SPOT so the arrow tip sits exactly at (x, y)
    cursorRef.current.style.transform = `translate(${x - HOT_SPOT}px, ${y - HOT_SPOT}px)`;

    // Update circular trail buffer
    trailBuffer.current.push({ x, y });
    if (trailBuffer.current.length > TRAIL_COUNT) {
      trailBuffer.current.shift();
    }

    // Reposition trail dots (oldest = most transparent, furthest behind)
    const isMoving = stateRef.current === "moving";
    trailRefs.current.forEach((el, i) => {
      if (!el) return;
      const pos = trailBuffer.current[trailBuffer.current.length - 1 - i];
      if (pos && isMoving) {
        el.style.transform = `translate(${pos.x - 4}px, ${pos.y - 4}px)`;
        el.style.opacity = String(((TRAIL_COUNT - i) / TRAIL_COUNT) * 0.22);
      } else {
        el.style.opacity = "0";
      }
    });
  };

  const revealCursor = () => {
    if (hideTimer.current) {
      clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    if (cursorRef.current) cursorRef.current.style.opacity = "1";
  };

  const scheduleFade = (delayMs: number) => {
    if (hideTimer.current) clearTimeout(hideTimer.current);
    hideTimer.current = setTimeout(() => {
      if (cursorRef.current) cursorRef.current.style.opacity = "0";
      // Clear trail dots
      trailBuffer.current = [];
      trailRefs.current.forEach((el) => {
        if (el) el.style.opacity = "0";
      });
    }, delayMs);
  };

  const fireRipple = (x: number, y: number, color: string) => {
    const idx = nextRippleIdx.current % rippleRefs.current.length;
    nextRippleIdx.current++;
    const el = rippleRefs.current[idx];
    if (!el) return;

    // Position via left/top so the CSS animation can freely use transform:scale
    el.style.left = `${x - 20}px`;
    el.style.top = `${y - 20}px`;
    el.style.borderColor = color;
    // Transparent fill matching the click color
    el.style.backgroundColor = `${color}18`;

    // Remove → read offsetWidth (forces synchronous layout, committing the removal
    // to the browser before re-adding) → add. Without the forced reflow, browsers
    // batch-optimize the remove+add into a no-op and the animation never resets.
    el.classList.remove("juno-ripple-active");
    void el.offsetWidth; // intentional forced reflow — do not remove
    el.classList.add("juno-ripple-active");
  };

  // ── Window setup (runs once on mount) ────────────────────────────────────
  useEffect(() => {
    let mounted = true;

    const setupWindow = async () => {
      try {
        const win = getCurrentWindow();

        // Resize to cover the full primary screen, position at origin
        // Covers the primary monitor in logical (CSS) pixels.
        // Multi-monitor support (spanning to secondary displays) is a future enhancement.
        await Promise.all([
          win.setSize(
            new LogicalSize(window.screen.width, window.screen.height)
          ),
          win.setPosition(new PhysicalPosition(0, 0)),
          // Make the entire window click-through — interaction still handled by AX/CGEvent
          win.setIgnoreCursorEvents(true),
        ]);

        if (mounted) {
          // Show the window now — cursor sprite starts hidden (opacity: 0)
          // and is revealed on first cursor event, so the user never sees a flash
          await win.show();
        }
      } catch (err) {
        console.error("[JunoCursor] Window setup failed:", err);
      }
    };

    setupWindow();

    return () => {
      mounted = false;
      if (hideTimer.current) clearTimeout(hideTimer.current);
      if (clickTimer.current) clearTimeout(clickTimer.current);
    };
  }, []);

  // ── Cursor movement events ────────────────────────────────────────────────

  useEventListener<[number, number]>("ui-cursor-highlight-start", ([x, y]) => {
    revealCursor();
    moveCursorTo(x, y);
    applyState("moving");
  });

  useEventListener<[number, number]>("ui-cursor-highlight-move", ([x, y]) => {
    moveCursorTo(x, y);
    // Don't interrupt the click recoil animation mid-flight
    if (stateRef.current !== "clicking") applyState("moving");
  });

  useEventListener<[number, number]>("ui-cursor-highlight-stop", ([x, y]) => {
    moveCursorTo(x, y);
    applyState("idle");
    scheduleFade(CURSOR_FADE_DELAY_MS);
  });

  // ── Click visualization ───────────────────────────────────────────────────

  useEventListener<[number, number, string]>(
    "click-visualization",
    ([x, y, color]) => {
      revealCursor();
      moveCursorTo(x, y);
      fireRipple(x, y, color);
      applyState("clicking");

      if (clickTimer.current) clearTimeout(clickTimer.current);
      clickTimer.current = setTimeout(() => {
        applyState("idle");
        scheduleFade(CURSOR_FADE_DELAY_MS);
      }, CLICK_ANIM_DURATION_MS);
    }
  );

  // ── Agent thinking state ──────────────────────────────────────────────────
  // agent-thinking-start/end fire during Claude's extended thinking feature;
  // show cursor in "thinking" mode only when not already moving.

  useEventListener("agent-thinking-start", () => {
    if (stateRef.current === "idle") {
      revealCursor();
      applyState("thinking");
    }
  });

  useEventListener("agent-thinking-end", () => {
    if (stateRef.current === "thinking") {
      applyState("idle");
      scheduleFade(CURSOR_FADE_DELAY_MS);
    }
  });

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        pointerEvents: "none",
        overflow: "hidden",
        background: "transparent",
      }}
    >
      <style>{CURSOR_CSS}</style>

      {/* Motion trail: 5 small dots, updated imperatively during movement */}
      {Array.from({ length: TRAIL_COUNT }, (_, i) => (
        <div
          key={`trail-${i}`}
          ref={(el) => {
            trailRefs.current[i] = el;
          }}
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: "rgba(139, 92, 246, 0.65)",
            opacity: 0,
            transform: "translate(-200px, -200px)",
            pointerEvents: "none",
            willChange: "transform, opacity",
          }}
        />
      ))}

      {/* Click ripple pool: 5 elements reused in round-robin for rapid clicks */}
      {Array.from({ length: 5 }, (_, i) => (
        <div
          key={`ripple-${i}`}
          ref={(el) => {
            rippleRefs.current[i] = el;
          }}
          style={{
            position: "absolute",
            width: 40,
            height: 40,
            borderRadius: "50%",
            border: "2px solid",
            opacity: 0,
            pointerEvents: "none",
          }}
        />
      ))}

      {/* Juno cursor sprite — hot-spot offset baked into transform via moveCursorTo() */}
      <div
        ref={cursorRef}
        className="juno-cursor juno-cursor--idle"
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          opacity: 0, // hidden until first cursor event
          transform: "translate(-200px, -200px)",
          pointerEvents: "none",
        }}
      >
        <JunoCursorShape />
      </div>
    </div>
  );
};

export default DesktopCursorOverlay;
