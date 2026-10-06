import { useCallback, useEffect, useRef } from "react";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";

/**
 * The POINT teaching cursor: when the agent writes [POINT:x,y:label], an arrow
 * flies along a short arc to the point and shows the label. Moved out of
 * DesktopCursorOverlay unchanged, except that it now draws in window
 * coordinates (the overlay covers one display, not the union of all).
 */

const FLIGHT_DURATION = 380; // ms, bezier arc from last landed to target
const LINGER_DURATION = 1600; // ms, display time after landing before hiding

type CursorPointPayload = {
  x: number;
  y: number;
  label: string | null;
  screen: number | null;
};

// Quadratic bezier: P(t) = (1-t)²·P0 + 2(1-t)t·P1 + t²·P2
function bezier(t: number, p0: number, p1: number, p2: number): number {
  const mt = 1 - t;
  return mt * mt * p0 + 2 * mt * t * p1 + t * t * p2;
}

export const PointFlight = ({ originX, originY }: { originX: number; originY: number }) => {
  const flyDivRef = useRef<HTMLDivElement | null>(null);
  const flyLabelRef = useRef<HTMLDivElement | null>(null);
  const flyAnimFrameRef = useRef<number | null>(null);
  const flyLingerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastLandedRef = useRef<{ x: number; y: number } | null>(null);

  const flyTo = useCallback((targetX: number, targetY: number, label: string | null) => {
    if (flyAnimFrameRef.current !== null) cancelAnimationFrame(flyAnimFrameRef.current);
    if (flyLingerRef.current) clearTimeout(flyLingerRef.current);

    const startX = lastLandedRef.current?.x ?? targetX - 250;
    const startY = lastLandedRef.current?.y ?? targetY - 80;

    // Perpendicular bezier control point: 15% of flight distance, max 80px arc
    const midX = (startX + targetX) / 2;
    const midY = (startY + targetY) / 2;
    const dx = targetX - startX;
    const dy = targetY - startY;
    const dist = Math.sqrt(dx * dx + dy * dy);
    const perp = Math.min(0.15, 80 / Math.max(dist, 1));
    const ctrlX = midX - dy * perp;
    const ctrlY = midY + dx * perp;

    if (flyDivRef.current) {
      flyDivRef.current.style.opacity = "1";
      flyDivRef.current.style.transform = `translate(${startX}px, ${startY}px)`;
    }
    if (flyLabelRef.current) {
      flyLabelRef.current.style.opacity = "0";
      flyLabelRef.current.textContent = label ?? "";
    }

    const startTime = Date.now();
    const tick = () => {
      const elapsed = Date.now() - startTime;
      const tRaw = Math.min(elapsed / FLIGHT_DURATION, 1);
      const t = tRaw < 0.5 ? 4 * tRaw * tRaw * tRaw : 1 - Math.pow(-2 * tRaw + 2, 3) / 2;
      if (flyDivRef.current) {
        flyDivRef.current.style.transform = `translate(${bezier(t, startX, ctrlX, targetX)}px, ${bezier(t, startY, ctrlY, targetY)}px)`;
      }
      if (tRaw < 1) {
        flyAnimFrameRef.current = requestAnimationFrame(tick);
      } else {
        lastLandedRef.current = { x: targetX, y: targetY };
        if (label && flyLabelRef.current) flyLabelRef.current.style.opacity = "1";
        flyLingerRef.current = setTimeout(() => {
          if (flyDivRef.current) flyDivRef.current.style.opacity = "0";
          if (flyLabelRef.current) flyLabelRef.current.style.opacity = "0";
        }, LINGER_DURATION);
      }
    };
    flyAnimFrameRef.current = requestAnimationFrame(tick);
  }, []);

  useEffect(
    () => () => {
      if (flyAnimFrameRef.current !== null) cancelAnimationFrame(flyAnimFrameRef.current);
      if (flyLingerRef.current) clearTimeout(flyLingerRef.current);
    },
    [],
  );

  useEventListener<CursorPointPayload>(EVENTS.UI_CURSOR_POINT, (payload) => {
    flyTo(payload.x - originX, payload.y - originY, payload.label ?? null);
  });

  return (
    <div
      ref={flyDivRef}
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        opacity: 0,
        transform: "translate(-200px, -200px)",
        pointerEvents: "none",
        willChange: "transform, opacity",
        transition: "opacity 0.24s ease",
      }}
    >
      <svg width="36" height="36" viewBox="0 0 36 36" fill="none" aria-hidden="true">
        <path
          d="M6 4L28 18L17 19.5L12 30L6 4Z"
          fill="white"
          stroke="#1a1a1a"
          strokeWidth="2"
          strokeLinejoin="round"
        />
      </svg>
      <div
        ref={flyLabelRef}
        style={{
          position: "absolute",
          top: "40px",
          left: 0,
          backgroundColor: "rgba(30, 30, 30, 0.88)",
          color: "#f5f5f7",
          fontSize: "12px",
          fontWeight: 500,
          fontFamily: "-apple-system, system-ui, sans-serif",
          padding: "4px 10px",
          borderRadius: "6px",
          whiteSpace: "nowrap",
          opacity: 0,
          transition: "opacity 0.18s ease-out",
        }}
      />
    </div>
  );
};
