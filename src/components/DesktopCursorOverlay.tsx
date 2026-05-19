import { useEffect, useReducer, useRef } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import { useEventListener } from "@/hooks/useEventListener";

type CursorState = "idle" | "moving" | "clicking" | "thinking";

// Maximum number of simultaneous agent cursors rendered.
const MAX_AGENT_SLOTS = 8;
const TRAIL_COUNT = 5;
// Hot-spot matches cx/cy of the tip circle in JunoCursorShape (SVG viewBox coords).
const HOT_SPOT = 5;
const CURSOR_FADE_DELAY_MS = 1500;
const CLICK_ANIM_DURATION_MS = 700;

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

  // ── Window setup ──────────────────────────────────────────────────────────
  useEffect(() => {
    let mounted = true;
    const setupWindow = async () => {
      try {
        const win = getCurrentWindow();
        await Promise.all([
          win.setSize(new LogicalSize(window.screen.width, window.screen.height)),
          win.setPosition(new PhysicalPosition(0, 0)),
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
    </div>
  );
};

export default DesktopCursorOverlay;
