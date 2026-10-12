import { useEffect, useReducer, useRef, useState } from "react";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";
import {
  ARROW_SHAPE,
  FADE_IN_MS,
  FADE_OUT_MS,
  FOREGROUND_LINGER_MS,
  GHOST_IDLE_MS,
  GLIDE_MS,
  INITIAL_OVERLAY_STATE,
  PULSE_MS,
  cursorBox,
  overlayReducer,
  shapeFor,
  type AgentCursorUpdate,
  type CursorShape,
  type DrawnCursor,
} from "@/lib/agentCursor";
import { PointFlight } from "./PointFlight";

/**
 * Juno's cursor, drawn in the `desktop-cursor-overlay` window: a soft glow
 * behind the real cursor when Juno moves it, or a ghost arrow wearing the same
 * glow where Juno works in the background. Rust owns the window (which display
 * it covers, when it shows, its level) and every fact drawn here; see
 * `src-tauri/src/cursor_overlay.rs`. The geometry is in `@/lib/agentCursor`.
 */

const EASE_OUT = "cubic-bezier(0.2, 0, 0, 1)";

const CURSOR_CSS = `
  .juno-agent-cursor {
    position: absolute;
    left: 0;
    top: 0;
    pointer-events: none;
    will-change: transform, opacity;
    animation: juno-cursor-in ${FADE_IN_MS}ms ease-out;
  }
  @keyframes juno-cursor-in {
    from { opacity: 0; }
  }

  /* Two blurred silhouettes of the cursor, tinted: a tight core that traces
     the outline and a wide, faint bloom. The blur sits on a wrapper because a
     filter on the masked element itself would be clipped back to the shape. */
  .juno-glow, .juno-glow-layer, .juno-glow-tint {
    position: absolute;
    inset: 0;
  }
  .juno-glow-tint {
    -webkit-mask-repeat: no-repeat;
    mask-repeat: no-repeat;
    -webkit-mask-size: 100% 100%;
    mask-size: 100% 100%;
  }

  .juno-glow--pulse { animation: juno-glow-pulse ${PULSE_MS}ms ${EASE_OUT}; }
  @keyframes juno-glow-pulse {
    0%   { transform: scale(1);    opacity: 1; }
    30%  { transform: scale(1.22); opacity: 1; }
    100% { transform: scale(1);    opacity: 1; }
  }
  .juno-ghost-arrow { position: absolute; inset: 0; display: block; }
  .juno-ghost-arrow--pulse { animation: juno-ghost-press 200ms ${EASE_OUT}; }
  @keyframes juno-ghost-press {
    0%   { transform: scale(1); }
    35%  { transform: scale(0.9); }
    100% { transform: scale(1); }
  }

  /* Reduced motion: no glide, no scale. The fades stay, and a click still
     shows, as a brief brightening. */
  @media (prefers-reduced-motion: reduce) {
    .juno-agent-cursor { transition-property: opacity !important; }
    .juno-glow--pulse { animation: juno-glow-flash ${PULSE_MS}ms ease-out; }
    .juno-ghost-arrow--pulse { animation: none; }
  }
  @keyframes juno-glow-flash {
    0%, 100% { opacity: 1; }
    30%      { opacity: 0.55; }
  }
`;

function GlowLayer({
  shape,
  color,
  blur,
  scale,
  opacity,
}: {
  shape: CursorShape;
  color: string;
  blur: number;
  scale: number;
  opacity: number;
}) {
  const mask = `url("${shape.image}")`;
  return (
    <div className="juno-glow-layer" style={{ filter: `blur(${blur}px)`, opacity }}>
      <div
        className="juno-glow-tint"
        style={{
          backgroundColor: color,
          WebkitMaskImage: mask,
          maskImage: mask,
          transform: `scale(${scale})`,
          transformOrigin: `${shape.hotspot_x}px ${shape.hotspot_y}px`,
        }}
      />
    </div>
  );
}

function AgentCursor({
  cursor,
  originX,
  originY,
  systemShape,
}: {
  cursor: DrawnCursor;
  originX: number;
  originY: number;
  systemShape: CursorShape | null;
}) {
  const shape = shapeFor(cursor.look, systemShape);
  const box = cursorBox(cursor.x, cursor.y, originX, originY, shape);
  const fade = cursor.visible ? `opacity ${FADE_IN_MS}ms ease-out` : `opacity ${FADE_OUT_MS}ms ease-in`;
  const transition = cursor.instant ? fade : `${fade}, transform ${GLIDE_MS}ms ${EASE_OUT}`;
  const pulsing = cursor.pulse > 0;
  const origin = `${box.hotspotX}px ${box.hotspotY}px`;

  return (
    <div
      className="juno-agent-cursor"
      data-agent-cursor={cursor.id}
      data-look={cursor.look}
      data-visible={cursor.visible ? "true" : "false"}
      style={{
        width: box.width,
        height: box.height,
        transform: `translate3d(${box.left}px, ${box.top}px, 0)`,
        opacity: cursor.visible ? 1 : 0,
        transition,
      }}
    >
      {/* Remounted per click, so the pulse restarts. */}
      <div
        key={`glow-${cursor.pulse}`}
        className={pulsing ? "juno-glow juno-glow--pulse" : "juno-glow"}
        style={{ transformOrigin: origin }}
      >
        <GlowLayer shape={shape} color={cursor.color} blur={10} scale={1.6} opacity={0.45} />
        <GlowLayer shape={shape} color={cursor.color} blur={3} scale={1.2} opacity={0.9} />
      </div>
      {cursor.look === "ghost" && (
        <img
          key={`ghost-${cursor.pulse}`}
          src={ARROW_SHAPE.image}
          width={ARROW_SHAPE.width}
          height={ARROW_SHAPE.height}
          alt=""
          aria-hidden="true"
          draggable={false}
          className={pulsing ? "juno-ghost-arrow juno-ghost-arrow--pulse" : "juno-ghost-arrow"}
          style={{ transformOrigin: origin }}
        />
      )}
    </div>
  );
}

export const DesktopCursorOverlay = () => {
  const [state, dispatch] = useReducer(overlayReducer, INITIAL_OVERLAY_STATE);
  const [systemShape, setSystemShape] = useState<CursorShape | null>(null);
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  const schedule = (id: string, ms: number, then: () => void) => {
    const existing = timers.current.get(id);
    if (existing) clearTimeout(existing);
    timers.current.set(
      id,
      setTimeout(() => {
        timers.current.delete(id);
        then();
      }, ms),
    );
  };

  useEffect(() => {
    const pending = timers.current;
    return () => {
      pending.forEach((t) => clearTimeout(t));
      pending.clear();
    };
  }, []);

  useEventListener<AgentCursorUpdate>(EVENTS.UI_AGENT_CURSOR_UPDATE, (update) => {
    dispatch({ type: "update", update });
    // The glow belongs to the real cursor, which the person takes back the
    // moment they move it, so it lingers only briefly. The ghost stays until
    // Juno lets go; the long timeout only covers a release that never came.
    const id = update.agent_id;
    schedule(id, update.foreground ? FOREGROUND_LINGER_MS : GHOST_IDLE_MS, () =>
      dispatch({ type: "fade", id }),
    );
  });

  useEventListener<{ agent_id: string }>(EVENTS.UI_AGENT_CURSOR_REMOVE, ({ agent_id }) => {
    dispatch({ type: "fade", id: agent_id });
    schedule(agent_id, FADE_OUT_MS, () => dispatch({ type: "remove", id: agent_id }));
  });

  useEventListener<CursorShape>(EVENTS.UI_AGENT_CURSOR_SHAPE, (shape) => {
    setSystemShape(shape);
  });

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
      {state.cursors.map((cursor) => (
        <AgentCursor
          key={cursor.id}
          cursor={cursor}
          originX={state.originX}
          originY={state.originY}
          systemShape={systemShape}
        />
      ))}
      <PointFlight originX={state.originX} originY={state.originY} />
    </div>
  );
};

export default DesktopCursorOverlay;
