/**
 * What the agent cursor overlay draws, as pure functions so it can be tested
 * without a window. Rust decides everything (`src-tauri/src/cursor_overlay.rs`):
 * where the overlay window sits, whether Juno moved the real cursor, and the
 * color. This file only turns those facts into boxes on the page.
 *
 * Two looks, one glow:
 * - foreground: Juno moved the person's real cursor. WindowServer draws that
 *   cursor above every window, so the overlay draws a soft tinted silhouette of
 *   the same shape just behind it: a halo around whatever the cursor is.
 * - background: Juno acted without touching the real cursor. The overlay draws
 *   a ghost arrow, wearing the same glow, at the point Juno acted on.
 */

/** Payload of `agent-cursor-update`. Field names match `AgentCursorState` in Rust. */
export interface AgentCursorUpdate {
  agent_id: string;
  /** Global logical points, top left of the primary display. */
  x: number;
  y: number;
  /** "idle" | "moving" | "clicking" */
  state: string;
  color: string;
  foreground?: boolean;
  /** Top left of the display the overlay window covers, same space as x/y. */
  origin_x?: number;
  origin_y?: number;
}

/** Payload of `agent-cursor-shape`. Field names match `CursorShape` in Rust. */
export interface CursorShape {
  image: string;
  hotspot_x: number;
  hotspot_y: number;
  width: number;
  height: number;
}

export type CursorLook = "glow" | "ghost";

export interface CursorBox {
  left: number;
  top: number;
  width: number;
  height: number;
  hotspotX: number;
  hotspotY: number;
}

/** A cursor the overlay is drawing. */
export interface DrawnCursor {
  id: string;
  /** Global point. */
  x: number;
  y: number;
  color: string;
  look: CursorLook;
  visible: boolean;
  /** Bumped on every click, so the pulse restarts. */
  pulse: number;
  /** Place without a glide: the first frame, or a jump the eye should not follow. */
  instant: boolean;
}

export interface OverlayState {
  cursors: DrawnCursor[];
  originX: number;
  originY: number;
}

export const INITIAL_OVERLAY_STATE: OverlayState = {
  cursors: [],
  originX: 0,
  originY: 0,
};

/** Fade in when Juno takes a cursor. */
export const FADE_IN_MS = 180;
/** Fade out when Juno lets go. Rust hides the window after `cursor_overlay::FADE_OUT_MS` (260). */
export const FADE_OUT_MS = 240;
/** The glow behind the real cursor stays this long after Juno's last move, then fades. */
export const FOREGROUND_LINGER_MS = 1200;
/** A ghost no release ever reached (a crashed run) fades after this much quiet. */
export const GHOST_IDLE_MS = 30_000;
/** The click pulse. */
export const PULSE_MS = 280;
/** How far the ghost glides between two points. */
export const GLIDE_MS = 180;

/**
 * The ghost arrow: the shape of the macOS arrow, black with a white keyline,
 * tip at the hotspot.
 */
const ARROW_PATH = "M3 2 L3 18.5 L7 14.8 L9.8 21.2 L12.6 20 L9.9 13.7 L15.2 13.7 Z";
export const ARROW_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="18" height="24" viewBox="0 0 18 24"><path d="${ARROW_PATH}" fill="black" stroke="white" stroke-width="1.5" stroke-linejoin="round"/></svg>`;
export const ARROW_SHAPE: CursorShape = {
  image: `data:image/svg+xml;utf8,${encodeURIComponent(ARROW_SVG)}`,
  hotspot_x: 3,
  hotspot_y: 2,
  width: 18,
  height: 24,
};

/** Which look an update asks for. */
export function lookFor(update: Pick<AgentCursorUpdate, "foreground">): CursorLook {
  return update.foreground ? "glow" : "ghost";
}

/** The shape to glow in: the real cursor's when Juno holds it, the ghost arrow otherwise. */
export function shapeFor(look: CursorLook, systemShape: CursorShape | null): CursorShape {
  if (look === "glow" && systemShape && systemShape.width > 0 && systemShape.height > 0) {
    return systemShape;
  }
  return ARROW_SHAPE;
}

/**
 * Where a cursor's box sits in the window, so its hotspot lands exactly on the
 * point: the global point, less the window's origin, less the hotspot.
 */
export function cursorBox(
  x: number,
  y: number,
  originX: number,
  originY: number,
  shape: CursorShape,
): CursorBox {
  return {
    left: x - originX - shape.hotspot_x,
    top: y - originY - shape.hotspot_y,
    width: shape.width,
    height: shape.height,
    hotspotX: shape.hotspot_x,
    hotspotY: shape.hotspot_y,
  };
}

export type OverlayAction =
  | { type: "update"; update: AgentCursorUpdate }
  | { type: "fade"; id: string }
  | { type: "remove"; id: string };

/** The overlay's state after one event. Pure; timers live in the component. */
export function overlayReducer(state: OverlayState, action: OverlayAction): OverlayState {
  switch (action.type) {
    case "update": {
      const u = action.update;
      const originX = u.origin_x ?? state.originX;
      const originY = u.origin_y ?? state.originY;
      const look = lookFor(u);
      const existing = state.cursors.find((c) => c.id === u.agent_id);
      const clicked = u.state === "clicking";
      const next: DrawnCursor = {
        id: u.agent_id,
        x: u.x,
        y: u.y,
        color: u.color,
        look,
        visible: true,
        pulse: (existing?.pulse ?? 0) + (clicked ? 1 : 0),
        // The glow rides the real cursor, which jumps, so it never glides. A
        // ghost glides only from a point the person could already see.
        instant:
          look === "glow" ||
          !existing ||
          !existing.visible ||
          existing.look !== look ||
          originX !== state.originX ||
          originY !== state.originY,
      };
      const cursors = existing
        ? state.cursors.map((c) => (c.id === u.agent_id ? next : c))
        : [...state.cursors, next];
      return { cursors, originX, originY };
    }
    case "fade":
      return {
        ...state,
        cursors: state.cursors.map((c) =>
          c.id === action.id ? { ...c, visible: false, instant: true } : c,
        ),
      };
    case "remove":
      return { ...state, cursors: state.cursors.filter((c) => c.id !== action.id) };
  }
}
