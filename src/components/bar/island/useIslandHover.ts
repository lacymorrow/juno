import { useCallback, useEffect, useRef, useState } from "react";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

/**
 * A mouse-leave is only believed after this long, once the cursor has been
 * checked against the window. Growing the window under a resting cursor (the
 * island opening its hover controls) makes AppKit/WebKit report a leave that
 * never happened. Same rule and number as the Pill.
 */
export const LEAVE_VERIFY_MS = 120;

/** Attribute that names a hover control, so the forwarded cursor can light it. */
export const ISLAND_BUTTON_ATTR = "data-island-button";

/** Is the cursor over this window right now? */
async function cursorInsideWindow(): Promise<boolean> {
  const w = getCurrentWindow();
  const [c, p, s] = await Promise.all([cursorPosition(), w.outerPosition(), w.outerSize()]);
  return c.x >= p.x && c.x < p.x + s.width && c.y >= p.y && c.y < p.y + s.height;
}

export interface IslandHover {
  /** The pointer is over the island's window. */
  hovered: boolean;
  /** Which hover control the pointer is over, for when CSS :hover cannot see it. */
  hoveredButton: string | null;
  /** Spread onto the root, for the active-app case. */
  pointerProps: {
    onPointerEnter: () => void;
    onPointerLeave: () => void;
  };
}

/**
 * Hover for the Island, from two sources.
 *
 * DOM pointer events only reach the web view while Juno is the active app,
 * which is rarely when someone glances at the island. The native tracking area
 * on the bar window reports enter, leave and move whatever app is active (the
 * same three events the Pill listens to), so both feed one state. A move is
 * hit-tested here, because macOS does not route mouse-moved into an inactive
 * window's web view and CSS :hover never fires there.
 */
export function useIslandHover(): IslandHover {
  const [hovered, setHovered] = useState(false);
  const [hoveredButton, setHoveredButton] = useState<string | null>(null);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const enter = useCallback(() => {
    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
      leaveTimerRef.current = null;
    }
    setHovered(true);
  }, []);

  const leave = useCallback(() => {
    if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
    leaveTimerRef.current = setTimeout(async () => {
      leaveTimerRef.current = null;
      let inside = false;
      try {
        inside = await cursorInsideWindow();
      } catch {
        inside = false;
      }
      if (!inside) {
        setHovered(false);
        setHoveredButton(null);
      }
    }, LEAVE_VERIFY_MS);
  }, []);

  const moved = useCallback((payload: unknown) => {
    const p = payload as { x?: number; y?: number } | null;
    if (!p || typeof p.x !== "number" || typeof p.y !== "number") return;
    if (typeof document.elementFromPoint !== "function") return;
    const el = document.elementFromPoint(p.x, p.y);
    const button = el?.closest(`[${ISLAND_BUTTON_ATTR}]`);
    setHoveredButton(button?.getAttribute(ISLAND_BUTTON_ATTR) ?? null);
  }, []);

  useEventListener(EVENTS.SYSTEM_MOUSE_ENTERED_WINDOW, enter);
  useEventListener(EVENTS.SYSTEM_MOUSE_LEFT_WINDOW, leave);
  useEventListener(EVENTS.SYSTEM_MOUSE_MOVED_WINDOW, moved);

  useEffect(
    () => () => {
      if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
    },
    [],
  );

  return {
    hovered,
    hoveredButton,
    pointerProps: { onPointerEnter: enter, onPointerLeave: leave },
  };
}
