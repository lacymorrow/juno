import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";

type CursorState = "idle" | "moving" | "clicking" | "thinking";

// Maximum number of simultaneous agent cursors rendered.
const MAX_AGENT_SLOTS = 8;
const TRAIL_COUNT = 5;
// Hot-spot matches cx/cy of the tip circle in JunoCursorShape (SVG viewBox coords).
const HOT_SPOT = 5;
const CURSOR_FADE_DELAY_MS = 1500;
const CLICK_ANIM_DURATION_MS = 700;

// ─── POINT flying cursor ───────────────────────────────────────────────────────
const FLIGHT_DURATION = 380; // ms — bezier arc from last landed to target
const LINGER_DURATION = 1600; // ms — display time after landing before hiding

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

// ─── Onboarding cursor constants ─────────────────────────────────────────────
// Accent color (#8B5CF6) matches JunoCursorShape and the app's brand violet.
const ONBOARDING_ACCENT = "#8B5CF6";
// Pulsing ring: 24px base radius, 4px amplitude (expressed in CSS scale), 1.2 Hz (833ms period)
const RING_BASE_RADIUS = 24;
// RING_AMPLITUDE_PX = 4 is encoded in the CSS keyframe scale factor: 1.17 ≈ (24+4)/24
const RING_PERIOD_MS = 833;
// Speech bubble appears 200ms after cursor arrives, text streams at 25ms/char
const BUBBLE_APPEAR_DELAY_MS = 200;
const BUBBLE_CHAR_INTERVAL_MS = 25;

// ─── CSS Animations ───────────────────────────────────────────────────────────
const CURSOR_CSS = `
  .juno-cursor {
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
    will-change: transform, opacity;
    transition: opacity 0.35s ease;
  }

  .juno-cursor--idle svg {
    animation: juno-breathe 3s ease-in-out infinite;
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
  }

  .juno-cursor--thinking svg {
    animation: juno-wobble 0.55s ease-in-out infinite;
    transform-origin: ${HOT_SPOT}px ${HOT_SPOT}px;
  }

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

  @keyframes juno-ripple {
    0%   { transform: scale(0.2); opacity: 1; }
    100% { transform: scale(3.2); opacity: 0; }
  }
  .juno-ripple-active {
    animation: juno-ripple 0.7s ease-out forwards;
  }

  /* ── Onboarding cursor animations ─────────────────────────────────────── */

  /* Pulsing highlight ring at 1.2Hz (833ms period) */
  @keyframes onb-ring-pulse {
    0%, 100% { transform: scale(1);   opacity: 0.9; }
    50%       { transform: scale(1.17); opacity: 0.55; }
  }
  .onb-ring-active {
    animation: onb-ring-pulse ${RING_PERIOD_MS}ms ease-in-out infinite;
  }

  /* Bubble fade-in */
  @keyframes onb-bubble-in {
    from { opacity: 0; transform: translateY(6px) scale(0.96); }
    to   { opacity: 1; transform: translateY(0)   scale(1); }
  }
  .onb-bubble-visible {
    animation: onb-bubble-in 0.18s ease-out forwards;
  }

  /* Celebration: cursor spin */
  @keyframes onb-celebrate-spin {
    0%   { transform: rotate(0deg) scale(1); }
    30%  { transform: rotate(20deg) scale(1.15); }
    60%  { transform: rotate(-10deg) scale(1.1); }
    100% { transform: rotate(0deg) scale(1); }
  }
  .onb-celebrate-spin {
    animation: onb-celebrate-spin 0.55s ease-in-out forwards;
  }

  /* Glow burst ring */
  @keyframes onb-glow-burst {
    0%   { transform: scale(0.3); opacity: 1; }
    100% { transform: scale(2.8); opacity: 0; }
  }
  .onb-glow-burst-active {
    animation: onb-glow-burst 0.6s ease-out forwards;
  }

  /* Celebration particle */
  @keyframes onb-particle-fly {
    0%   { transform: translate(0, 0) scale(1); opacity: 1; }
    100% { opacity: 0; }
  }
`;

// ─── Cursor SVG ───────────────────────────────────────────────────────────────
// Arrow cursor: hot-spot at (5, 5). `color` controls the gradient stop.
const JunoCursorShape = ({ color = "#8B5CF6" }: { color?: string }) => {
  const gradId = `juno-body-${color.replace("#", "")}`;
  return (
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
        <linearGradient
          id={gradId}
          x1="5" y1="5" x2="30" y2="42"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor={color} stopOpacity="0.95" />
          <stop offset="100%" stopColor="rgba(12, 8, 55, 0.92)" />
        </linearGradient>
      </defs>

      {/* Drop shadow */}
      <path
        d="M5 5 L5 34 L13 25 L17.5 37 L22 35 L17.5 23 L30 23 Z"
        fill="rgba(0,0,0,0.32)"
        transform="translate(1.5, 1.5)"
      />
      {/* Arrow body */}
      <path
        d="M5 5 L5 34 L13 25 L17.5 37 L22 35 L17.5 23 L30 23 Z"
        fill={`url(#${gradId})`}
        stroke={color}
        strokeWidth="1.5"
        strokeLinejoin="round"
        filter="url(#juno-glow)"
        strokeOpacity="0.88"
      />
      {/* Hot-spot glow ring */}
      <circle cx="5" cy="5" r="4.5" fill={color} fillOpacity="0.45" filter="url(#juno-glow)" />
      <circle cx="5" cy="5" r="2" fill="white" />
    </svg>
  );
};

// ─── Per-slot cursor refs ─────────────────────────────────────────────────────
interface SlotRefs {
  cursor: HTMLDivElement | null;
  trails: (HTMLDivElement | null)[];
  ripples: (HTMLDivElement | null)[];
  hideTimer: ReturnType<typeof setTimeout> | null;
  clickTimer: ReturnType<typeof setTimeout> | null;
  trailBuffer: { x: number; y: number }[];
  state: CursorState;
  nextRippleIdx: number;
}

// ─── Reducer for active agent-slot mapping ────────────────────────────────────
type SlotMap = Map<string, number>; // agentId → slot index

type SlotAction =
  | { type: "assign"; agentId: string; slot: number }
  | { type: "release"; agentId: string };

function slotReducer(map: SlotMap, action: SlotAction): SlotMap {
  const next = new Map(map);
  if (action.type === "assign") {
    next.set(action.agentId, action.slot);
  } else {
    next.delete(action.agentId);
  }
  return next;
}

// ─── Component ────────────────────────────────────────────────────────────────

// AgentCursorUpdate payload from backend
interface AgentCursorUpdate {
  agent_id: string;
  x: number;
  y: number;
  state: string;
  color: string;
}

interface AgentCursorRemove {
  agent_id: string;
}

export const DesktopCursorOverlay = () => {
  // Slot assignment: agentId → 0..MAX_AGENT_SLOTS-1
  const [slotMap, dispatch] = useReducer(slotReducer, new Map<string, number>());

  // Per-slot imperative refs (indexed 0..MAX_AGENT_SLOTS-1)
  const slots = useRef<SlotRefs[]>(
    Array.from({ length: MAX_AGENT_SLOTS }, () => ({
      cursor: null,
      trails: Array<null>(TRAIL_COUNT).fill(null),
      ripples: Array<null>(5).fill(null),
      hideTimer: null,
      clickTimer: null,
      trailBuffer: [],
      state: "idle" as CursorState,
      nextRippleIdx: 0,
    }))
  );

  // Colors per slot (assigned at first AgentCursorUpdate, persisted for the session)
  const slotColors = useRef<(string | null)[]>(Array(MAX_AGENT_SLOTS).fill(null));

  // Tracks which slots are currently occupied
  const occupiedSlots = useRef<Set<number>>(new Set());

  // ── POINT flying cursor refs ───────────────────────────────────────────────
  const flyDivRef = useRef<HTMLDivElement | null>(null);
  const flyLabelRef = useRef<HTMLDivElement | null>(null);
  const flyRippleRef = useRef<HTMLDivElement | null>(null);
  const flyAnimFrameRef = useRef<number | null>(null);
  const flyLingerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastLandedRef = useRef<{ x: number; y: number } | null>(null);

  // ── Onboarding cursor refs (cursor-animation-frame driven) ─────────────────
  // All positioned imperatively to avoid 60fps React re-renders.
  const onbCursorRef = useRef<HTMLDivElement | null>(null);     // cursor sprite
  const onbRingRef = useRef<HTMLDivElement | null>(null);       // pulsing highlight ring
  const onbBubbleRef = useRef<HTMLDivElement | null>(null);     // speech bubble container
  const onbBubbleTextRef = useRef<HTMLSpanElement | null>(null);// streamed text inside bubble
  const onbGlowRef = useRef<HTMLDivElement | null>(null);       // celebration glow burst ring
  const onbParticlesRef = useRef<(HTMLDivElement | null)[]>([]); // 6 particle dots
  const onbBubbleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onbBubbleIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Current cursor position — bubble needs this to anchor itself
  const onbPosRef = useRef<{ x: number; y: number }>({ x: -300, y: -300 });

  // ── Slot allocation ────────────────────────────────────────────────────────
  const getOrAssignSlot = (agentId: string, map: SlotMap): number | null => {
    const existing = map.get(agentId);
    if (existing !== undefined) return existing;

    for (let i = 0; i < MAX_AGENT_SLOTS; i++) {
      if (!occupiedSlots.current.has(i)) {
        occupiedSlots.current.add(i);
        dispatch({ type: "assign", agentId, slot: i });
        return i;
      }
    }
    return null; // all slots full
  };

  // ── Imperative slot helpers ────────────────────────────────────────────────
  const applySlotState = (slot: SlotRefs, state: CursorState) => {
    slot.state = state;
    if (slot.cursor) {
      slot.cursor.className = `juno-cursor juno-cursor--${state}`;
    }
  };

  const moveSlotTo = (slot: SlotRefs, x: number, y: number) => {
    if (!slot.cursor) return;
    slot.cursor.style.transform = `translate(${x - HOT_SPOT}px, ${y - HOT_SPOT}px)`;

    slot.trailBuffer.push({ x, y });
    if (slot.trailBuffer.length > TRAIL_COUNT) slot.trailBuffer.shift();

    const isMoving = slot.state === "moving";
    slot.trails.forEach((el, i) => {
      if (!el) return;
      const pos = slot.trailBuffer[slot.trailBuffer.length - 1 - i];
      if (pos && isMoving) {
        el.style.transform = `translate(${pos.x - 4}px, ${pos.y - 4}px)`;
        el.style.opacity = String(((TRAIL_COUNT - i) / TRAIL_COUNT) * 0.22);
      } else {
        el.style.opacity = "0";
      }
    });
  };

  const revealSlot = (slot: SlotRefs) => {
    if (slot.hideTimer) { clearTimeout(slot.hideTimer); slot.hideTimer = null; }
    if (slot.cursor) slot.cursor.style.opacity = "1";
  };

  const scheduleSlotFade = (slot: SlotRefs, delayMs: number) => {
    if (slot.hideTimer) clearTimeout(slot.hideTimer);
    slot.hideTimer = setTimeout(() => {
      if (slot.cursor) slot.cursor.style.opacity = "0";
      slot.trailBuffer = [];
      slot.trails.forEach((el) => { if (el) el.style.opacity = "0"; });
    }, delayMs);
  };

  const fireSlotRipple = (slot: SlotRefs, x: number, y: number, color: string) => {
    const idx = slot.nextRippleIdx % slot.ripples.length;
    slot.nextRippleIdx++;
    const el = slot.ripples[idx];
    if (!el) return;
    el.style.left = `${x - 20}px`;
    el.style.top = `${y - 20}px`;
    el.style.borderColor = color;
    el.style.backgroundColor = `${color}18`;
    el.classList.remove("juno-ripple-active");
    void el.offsetWidth;
    el.classList.add("juno-ripple-active");
  };

  // ── POINT: fly cursor to (targetX, targetY) along a bezier arc ───────────
  const flyTo = useCallback((targetX: number, targetY: number, label: string | null) => {
    if (flyAnimFrameRef.current !== null) cancelAnimationFrame(flyAnimFrameRef.current);
    if (flyLingerRef.current) clearTimeout(flyLingerRef.current);

    const startX = lastLandedRef.current?.x ?? targetX - 250;
    const startY = lastLandedRef.current?.y ?? targetY - 80;

    // Perpendicular bezier control point — 15% of flight distance, max 80px arc
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
    if (flyRippleRef.current) {
      flyRippleRef.current.style.opacity = "0";
      flyRippleRef.current.classList.remove("juno-ripple-active");
    }

    const startTime = Date.now();
    const tick = () => {
      const elapsed = Date.now() - startTime;
      const tRaw = Math.min(elapsed / FLIGHT_DURATION, 1);
      // Ease-in-out cubic
      const t = tRaw < 0.5 ? 4 * tRaw * tRaw * tRaw : 1 - Math.pow(-2 * tRaw + 2, 3) / 2;
      const curX = bezier(t, startX, ctrlX, targetX);
      const curY = bezier(t, startY, ctrlY, targetY);
      if (flyDivRef.current) {
        flyDivRef.current.style.transform = `translate(${curX}px, ${curY}px)`;
      }
      if (tRaw < 1) {
        flyAnimFrameRef.current = requestAnimationFrame(tick);
      } else {
        lastLandedRef.current = { x: targetX, y: targetY };
        if (label && flyLabelRef.current) flyLabelRef.current.style.opacity = "1";
        if (flyRippleRef.current) {
          flyRippleRef.current.style.opacity = "1";
          flyRippleRef.current.classList.remove("juno-ripple-active");
          void flyRippleRef.current.offsetWidth;
          flyRippleRef.current.classList.add("juno-ripple-active");
        }
        flyLingerRef.current = setTimeout(() => {
          if (flyDivRef.current) flyDivRef.current.style.opacity = "0";
          if (flyLabelRef.current) flyLabelRef.current.style.opacity = "0";
        }, LINGER_DURATION);
      }
    };
    flyAnimFrameRef.current = requestAnimationFrame(tick);
  }, []);

  // ── Window setup ──────────────────────────────────────────────────────────
  // Phase D / LAC-1882: span the union of all connected monitors so the
  // onboarding cursor can fly to a System Settings window on a non-primary
  // display. Single-monitor users get the same coverage as before.
  useEffect(() => {
    let mounted = true;
    const setupWindow = async () => {
      try {
        const win = getCurrentWindow();
        // Compute the bounding rectangle that contains every monitor. Each
        // monitor has physical { position: {x,y}, size: {width,height} }.
        // We fall back to window.screen if the API errors (e.g. headless test).
        let originX = 0;
        let originY = 0;
        let spanWidth = window.screen.width;
        let spanHeight = window.screen.height;
        try {
          const monitors = await availableMonitors();
          if (monitors.length > 0) {
            let minX = Number.POSITIVE_INFINITY;
            let minY = Number.POSITIVE_INFINITY;
            let maxX = Number.NEGATIVE_INFINITY;
            let maxY = Number.NEGATIVE_INFINITY;
            for (const m of monitors) {
              // Tauri monitor coords are physical pixels; for setSize we use
              // logical pixels (assume scale factor 1 for span; the overlay
              // is transparent + pointer-events:none so an oversize window
              // is harmless).
              const scale = m.scaleFactor || 1;
              const lx = m.position.x / scale;
              const ly = m.position.y / scale;
              const lw = m.size.width / scale;
              const lh = m.size.height / scale;
              if (lx < minX) minX = lx;
              if (ly < minY) minY = ly;
              if (lx + lw > maxX) maxX = lx + lw;
              if (ly + lh > maxY) maxY = ly + lh;
            }
            if (Number.isFinite(minX) && Number.isFinite(maxX)) {
              originX = minX;
              originY = minY;
              spanWidth = maxX - minX;
              spanHeight = maxY - minY;
            }
          }
        } catch (err) {
          console.debug("[JunoCursor] availableMonitors failed; using primary display:", err);
        }

        await Promise.all([
          win.setSize(new LogicalSize(spanWidth, spanHeight)),
          // PhysicalPosition is in raw pixels; multiplying by primary scale is
          // approximate, but Tauri normalizes by primary monitor's scale.
          win.setPosition(new PhysicalPosition(Math.round(originX), Math.round(originY))),
          win.setIgnoreCursorEvents(true),
        ]);
        if (mounted) await win.show();
      } catch (err) {
        console.error("[JunoCursor] Window setup failed:", err);
      }
    };
    setupWindow();
    return () => {
      mounted = false;
      slots.current.forEach((slot) => {
        if (slot.hideTimer) clearTimeout(slot.hideTimer);
        if (slot.clickTimer) clearTimeout(slot.clickTimer);
      });
      if (flyAnimFrameRef.current !== null) cancelAnimationFrame(flyAnimFrameRef.current);
      if (flyLingerRef.current) clearTimeout(flyLingerRef.current);
      if (onbBubbleTimerRef.current) clearTimeout(onbBubbleTimerRef.current);
      if (onbBubbleIntervalRef.current) clearInterval(onbBubbleIntervalRef.current);
    };
  }, []);

  // ── Multi-agent cursor events ─────────────────────────────────────────────
  useEventListener<AgentCursorUpdate>("agent-cursor-update", (payload) => {
    const slotIdx = getOrAssignSlot(payload.agent_id, slotMap);
    if (slotIdx === null) return; // all slots occupied

    // Store the color for this slot if not yet set
    if (!slotColors.current[slotIdx]) {
      slotColors.current[slotIdx] = payload.color;
    }

    const slot = slots.current[slotIdx];
    revealSlot(slot);
    moveSlotTo(slot, payload.x, payload.y);

    const state = (payload.state as CursorState) || "idle";
    if (state === "clicking") {
      applySlotState(slot, "clicking");
      fireSlotRipple(slot, payload.x, payload.y, payload.color);
      if (slot.clickTimer) clearTimeout(slot.clickTimer);
      slot.clickTimer = setTimeout(() => {
        applySlotState(slot, "idle");
        scheduleSlotFade(slot, CURSOR_FADE_DELAY_MS);
      }, CLICK_ANIM_DURATION_MS);
    } else {
      applySlotState(slot, state);
      if (state === "idle") scheduleSlotFade(slot, CURSOR_FADE_DELAY_MS);
    }
  });

  useEventListener<AgentCursorRemove>("agent-cursor-remove", ({ agent_id }) => {
    const slotIdx = slotMap.get(agent_id);
    if (slotIdx === undefined) return;

    const slot = slots.current[slotIdx];
    if (slot.cursor) slot.cursor.style.opacity = "0";
    slot.trailBuffer = [];
    slot.trails.forEach((el) => { if (el) el.style.opacity = "0"; });
    if (slot.hideTimer) { clearTimeout(slot.hideTimer); slot.hideTimer = null; }
    if (slot.clickTimer) { clearTimeout(slot.clickTimer); slot.clickTimer = null; }
    slotColors.current[slotIdx] = null;
    occupiedSlots.current.delete(slotIdx);
    dispatch({ type: "release", agentId: agent_id });
  });

  // ── Legacy single-agent cursor events (backward compat) ──────────────────
  // These events are emitted by smooth_mouse_move for the HID path.
  // They map to slot 0 with the default purple color.
  const LEGACY_SLOT = 0;

  useEventListener<[number, number]>("ui-cursor-highlight-start", ([x, y]) => {
    if (!occupiedSlots.current.has(LEGACY_SLOT)) occupiedSlots.current.add(LEGACY_SLOT);
    const slot = slots.current[LEGACY_SLOT];
    revealSlot(slot);
    moveSlotTo(slot, x, y);
    applySlotState(slot, "moving");
  });

  useEventListener<[number, number]>("ui-cursor-highlight-move", ([x, y]) => {
    const slot = slots.current[LEGACY_SLOT];
    moveSlotTo(slot, x, y);
    if (slot.state !== "clicking") applySlotState(slot, "moving");
  });

  useEventListener<[number, number]>("ui-cursor-highlight-stop", ([x, y]) => {
    const slot = slots.current[LEGACY_SLOT];
    moveSlotTo(slot, x, y);
    applySlotState(slot, "idle");
    scheduleSlotFade(slot, CURSOR_FADE_DELAY_MS);
  });

  useEventListener<[number, number, string]>("click-visualization", ([x, y, color]) => {
    const slot = slots.current[LEGACY_SLOT];
    revealSlot(slot);
    moveSlotTo(slot, x, y);
    fireSlotRipple(slot, x, y, color);
    applySlotState(slot, "clicking");
    if (slot.clickTimer) clearTimeout(slot.clickTimer);
    slot.clickTimer = setTimeout(() => {
      applySlotState(slot, "idle");
      scheduleSlotFade(slot, CURSOR_FADE_DELAY_MS);
    }, CLICK_ANIM_DURATION_MS);
  });

  useEventListener("agent-thinking-start", () => {
    const slot = slots.current[LEGACY_SLOT];
    if (slot.state === "idle") { revealSlot(slot); applySlotState(slot, "thinking"); }
  });

  useEventListener("agent-thinking-end", () => {
    const slot = slots.current[LEGACY_SLOT];
    if (slot.state === "thinking") { applySlotState(slot, "idle"); scheduleSlotFade(slot, CURSOR_FADE_DELAY_MS); }
  });

  // ── POINT teaching cursor — agent [POINT:x,y:label:screenN] ──────────────
  // payload.screen is parsed + forwarded but coordinates are treated as global
  // screen space. TODO: map screenN to display origin for multi-monitor support.
  useEventListener<CursorPointPayload>(EVENTS.UI_CURSOR_POINT, (payload) => {
    flyTo(payload.x, payload.y, payload.label ?? null);
  });

  // ── Onboarding cursor — cursor-animation-frame at 60fps ───────────────────
  // Backend emits {x, y, t, style} via animate_cursor_to. We move the onboarding
  // cursor sprite imperatively so React never re-renders at 60fps.
  //
  // Phase D / LAC-1882: under `prefers-reduced-motion: reduce` we honor the
  // backend's animation by only applying the final frame (`t === 1`). The
  // cursor effectively teleports to the destination — no Bezier sweep, no
  // intermediate motion. Backend still emits all frames; we just discard them.
  useEventListener<{ x: number; y: number; t: number; style: string }>(
    EVENTS.CURSOR_ANIMATION_FRAME,
    ({ x, y, t }) => {
      const reducedMotion =
        typeof window !== "undefined" &&
        window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

      // Under reduced motion, only the final frame (t=1) lands the cursor.
      // Skipping intermediate frames also drops the per-event DOM writes.
      if (reducedMotion && t < 1) {
        return;
      }

      onbPosRef.current = { x, y };

      // Cursor sprite
      if (onbCursorRef.current) {
        onbCursorRef.current.style.opacity = "1";
        onbCursorRef.current.style.transform = `translate(${x - HOT_SPOT}px, ${y - HOT_SPOT}px)`;
      }
      // Highlight ring follows cursor
      if (onbRingRef.current) {
        const r = RING_BASE_RADIUS;
        onbRingRef.current.style.left = `${x - r}px`;
        onbRingRef.current.style.top = `${y - r}px`;
      }
      // Bubble follows cursor (above-right of cursor tip)
      if (onbBubbleRef.current) {
        onbBubbleRef.current.style.left = `${x + 20}px`;
        onbBubbleRef.current.style.top = `${y - 48}px`;
      }
    }
  );

  // ── Onboarding highlight ring ─────────────────────────────────────────────
  useEventListener<{ x: number; y: number; radius: number }>(
    EVENTS.CURSOR_HIGHLIGHT,
    ({ x, y, radius }) => {
      const r = radius || RING_BASE_RADIUS;
      if (!onbRingRef.current) return;
      const el = onbRingRef.current;
      el.style.width = `${r * 2}px`;
      el.style.height = `${r * 2}px`;
      el.style.left = `${x - r}px`;
      el.style.top = `${y - r}px`;
      el.style.opacity = "1";
      el.classList.remove("onb-ring-active");
      void el.offsetWidth; // force reflow
      el.classList.add("onb-ring-active");
    }
  );

  // ── Onboarding speech bubble ──────────────────────────────────────────────
  // Appears BUBBLE_APPEAR_DELAY_MS after the event, then streams text at
  // BUBBLE_CHAR_INTERVAL_MS per character.
  useEventListener<{ x: number; y: number; text: string }>(
    EVENTS.CURSOR_BUBBLE,
    ({ x, y, text }) => {
      // Cancel any in-flight bubble timers
      if (onbBubbleTimerRef.current) clearTimeout(onbBubbleTimerRef.current);
      if (onbBubbleIntervalRef.current) clearInterval(onbBubbleIntervalRef.current);

      if (onbBubbleTextRef.current) onbBubbleTextRef.current.textContent = "";
      if (onbBubbleRef.current) {
        const el = onbBubbleRef.current;
        el.style.opacity = "0";
        el.style.left = `${x + 20}px`;
        el.style.top = `${y - 48}px`;
        el.classList.remove("onb-bubble-visible");
      }

      onbBubbleTimerRef.current = setTimeout(() => {
        if (!onbBubbleRef.current || !onbBubbleTextRef.current) return;
        onbBubbleRef.current.classList.add("onb-bubble-visible");

        let charIdx = 0;
        onbBubbleIntervalRef.current = setInterval(() => {
          if (!onbBubbleTextRef.current) return;
          if (charIdx >= text.length) {
            if (onbBubbleIntervalRef.current) clearInterval(onbBubbleIntervalRef.current);
            return;
          }
          onbBubbleTextRef.current.textContent = text.slice(0, ++charIdx);
        }, BUBBLE_CHAR_INTERVAL_MS);
      }, BUBBLE_APPEAR_DELAY_MS);
    }
  );

  // ── Dismiss cursor overlay ────────────────────────────────────────────────
  useEventListener<{ animate: boolean }>(EVENTS.CURSOR_DISMISS_OVERLAY, () => {
    if (onbBubbleTimerRef.current) clearTimeout(onbBubbleTimerRef.current);
    if (onbBubbleIntervalRef.current) clearInterval(onbBubbleIntervalRef.current);

    // Fade cursor sprite
    if (onbCursorRef.current) onbCursorRef.current.style.opacity = "0";
    // Fade ring
    if (onbRingRef.current) {
      onbRingRef.current.style.opacity = "0";
      onbRingRef.current.classList.remove("onb-ring-active");
    }
    // Fade bubble
    if (onbBubbleRef.current) {
      onbBubbleRef.current.style.opacity = "0";
      onbBubbleRef.current.classList.remove("onb-bubble-visible");
    }
  });

  // ── Celebration micro-animation ───────────────────────────────────────────
  // Backend (Phase C) can emit this after onboarding complete.
  // Spin the cursor, fire a glow ring, and shoot 6 particle dots.
  useEventListener<{ x?: number; y?: number }>("cursor-celebration", (payload) => {
    const cx = payload?.x ?? onbPosRef.current.x;
    const cy = payload?.y ?? onbPosRef.current.y;

    // Spin cursor sprite
    if (onbCursorRef.current) {
      const el = onbCursorRef.current;
      el.classList.remove("onb-celebrate-spin");
      void el.offsetWidth;
      el.classList.add("onb-celebrate-spin");
    }

    // Glow burst ring
    if (onbGlowRef.current) {
      const el = onbGlowRef.current;
      const r = 32;
      el.style.left = `${cx - r}px`;
      el.style.top = `${cy - r}px`;
      el.style.width = `${r * 2}px`;
      el.style.height = `${r * 2}px`;
      el.style.opacity = "1";
      el.classList.remove("onb-glow-burst-active");
      void el.offsetWidth;
      el.classList.add("onb-glow-burst-active");
    }

    // Particle dots — 6 directions
    const ANGLES = [0, 60, 120, 180, 240, 300];
    onbParticlesRef.current.forEach((el, i) => {
      if (!el) return;
      const angle = (ANGLES[i] ?? 0) * (Math.PI / 180);
      const dist = 48 + Math.random() * 24;
      const tx = Math.cos(angle) * dist;
      const ty = Math.sin(angle) * dist;
      el.style.left = `${cx - 4}px`;
      el.style.top = `${cy - 4}px`;
      el.style.opacity = "1";
      el.style.setProperty("--tx", `${tx}px`);
      el.style.setProperty("--ty", `${ty}px`);
      el.style.animation = "none";
      void el.offsetWidth;
      el.style.animation = `onb-particle-fly 0.6s ease-out ${i * 40}ms forwards`;
      el.style.transform = `translate(var(--tx), var(--ty))`;
    });
  });

  // ── Render: N cursor sprites ───────────────────────────────────────────────

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

      {Array.from({ length: MAX_AGENT_SLOTS }, (_, slotIdx) => {
        // Derive color: check slotColors ref first, fall back to palette
        const color = slotColors.current[slotIdx] ?? "#8B5CF6";

        return (
          <div key={`cursor-slot-${slotIdx}`}>
            {/* Motion trail dots */}
            {Array.from({ length: TRAIL_COUNT }, (__, i) => (
              <div
                key={`trail-${slotIdx}-${i}`}
                ref={(el) => { slots.current[slotIdx].trails[i] = el; }}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: `${color}a6`,
                  opacity: 0,
                  transform: "translate(-200px, -200px)",
                  pointerEvents: "none",
                  willChange: "transform, opacity",
                }}
              />
            ))}

            {/* Click ripple pool */}
            {Array.from({ length: 5 }, (__, i) => (
              <div
                key={`ripple-${slotIdx}-${i}`}
                ref={(el) => { slots.current[slotIdx].ripples[i] = el; }}
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

            {/* Cursor sprite */}
            <div
              ref={(el) => { slots.current[slotIdx].cursor = el; }}
              className="juno-cursor juno-cursor--idle"
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                opacity: 0,
                transform: "translate(-200px, -200px)",
                pointerEvents: "none",
              }}
            >
              <JunoCursorShape color={color} />
            </div>
          </div>
        );
      })}

      {/* ── Onboarding cursor overlay ────────────────────────────────────── */}

      {/* Onboarding cursor sprite — backend moves this via cursor-animation-frame */}
      <div
        ref={onbCursorRef}
        className="juno-cursor juno-cursor--idle"
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          opacity: 0,
          transform: "translate(-300px, -300px)",
          pointerEvents: "none",
          transition: "opacity 0.4s ease",
        }}
      >
        <JunoCursorShape color={ONBOARDING_ACCENT} />
      </div>

      {/* Pulsing highlight ring — shown by cursor-highlight event */}
      <div
        ref={onbRingRef}
        style={{
          position: "absolute",
          borderRadius: "50%",
          border: `3px solid ${ONBOARDING_ACCENT}`,
          boxShadow: `0 0 12px 3px ${ONBOARDING_ACCENT}55`,
          opacity: 0,
          pointerEvents: "none",
          width: RING_BASE_RADIUS * 2,
          height: RING_BASE_RADIUS * 2,
          top: -300,
          left: -300,
          transition: "opacity 0.25s ease",
        }}
      />

      {/* Speech bubble — shown by cursor-bubble event */}
      <div
        ref={onbBubbleRef}
        style={{
          position: "absolute",
          opacity: 0,
          top: -300,
          left: -300,
          pointerEvents: "none",
          maxWidth: 240,
          backgroundColor: "rgba(15, 12, 45, 0.92)",
          backdropFilter: "blur(10px)",
          border: `1px solid ${ONBOARDING_ACCENT}55`,
          borderRadius: 10,
          padding: "7px 12px",
          boxShadow: `0 4px 20px rgba(0,0,0,0.4), 0 0 0 1px ${ONBOARDING_ACCENT}22`,
          // Triangle pointer at bottom-left
          filter: "drop-shadow(0 2px 8px rgba(0,0,0,0.3))",
        }}
      >
        <span
          ref={onbBubbleTextRef}
          style={{
            color: "#e2e8f0",
            fontSize: 12,
            fontWeight: 500,
            fontFamily: "system-ui, -apple-system, sans-serif",
            lineHeight: 1.5,
            whiteSpace: "pre-wrap",
          }}
        />
      </div>

      {/* Celebration glow burst ring */}
      <div
        ref={onbGlowRef}
        style={{
          position: "absolute",
          borderRadius: "50%",
          border: `2px solid ${ONBOARDING_ACCENT}`,
          backgroundColor: `${ONBOARDING_ACCENT}22`,
          opacity: 0,
          pointerEvents: "none",
          top: -300,
          left: -300,
        }}
      />

      {/* Celebration particle dots — 6 of them */}
      {Array.from({ length: 6 }, (_, i) => (
        <div
          key={`onb-particle-${i}`}
          ref={(el) => { onbParticlesRef.current[i] = el; }}
          style={{
            position: "absolute",
            width: 6,
            height: 6,
            borderRadius: "50%",
            backgroundColor: i % 2 === 0 ? ONBOARDING_ACCENT : "#C4B5FD",
            opacity: 0,
            pointerEvents: "none",
            top: -300,
            left: -300,
          }}
        />
      ))}

      {/* POINT teaching cursor — flies to agent-pointed coordinates */}
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
        }}
      >
        <svg
          width="36"
          height="36"
          viewBox="0 0 36 36"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden="true"
        >
          <defs>
            <filter id="point-shadow" x="-20%" y="-20%" width="140%" height="140%">
              <feDropShadow dx="1" dy="2" stdDeviation="2" floodColor="rgba(0,0,0,0.4)" />
            </filter>
          </defs>
          <path
            d="M6 4L28 18L17 19.5L12 30L6 4Z"
            fill="white"
            stroke="#1a1a2e"
            strokeWidth="2"
            strokeLinejoin="round"
            filter="url(#point-shadow)"
          />
          <circle cx="6.5" cy="4.5" r="2.5" fill="#6366f1" />
        </svg>

        {/* Label tooltip — fades in on landing */}
        <div
          ref={flyLabelRef}
          style={{
            position: "absolute",
            top: "40px",
            left: 0,
            backgroundColor: "rgba(15, 15, 30, 0.88)",
            backdropFilter: "blur(8px)",
            color: "#e2e8f0",
            fontSize: "12px",
            fontWeight: 500,
            fontFamily: "system-ui, -apple-system, sans-serif",
            padding: "4px 10px",
            borderRadius: "6px",
            border: "1px solid rgba(99,102,241,0.4)",
            whiteSpace: "nowrap",
            opacity: 0,
            transition: "opacity 0.18s ease-out",
            boxShadow: "0 2px 12px rgba(0,0,0,0.3)",
          }}
        />

        {/* Landing ripple — reuses .juno-ripple-active keyframe */}
        <div
          ref={flyRippleRef}
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: 40,
            height: 40,
            transform: "translate(-8px, -8px)",
            borderRadius: "50%",
            border: "2px solid rgba(99,102,241,0.6)",
            opacity: 0,
            pointerEvents: "none",
          }}
        />
      </div>
    </div>
  );
};

export default DesktopCursorOverlay;
