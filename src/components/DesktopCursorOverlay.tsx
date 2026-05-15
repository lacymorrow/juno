import { useEffect, useState, useRef, useCallback } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";

type CursorHighlight = {
  x: number;
  y: number;
  active: boolean;
  timestamp: number;
};

type ClickVisualization = {
  x: number;
  y: number;
  color: string;
  id: number;
  timestamp: number;
};

type FlyingCursor = {
  label: string | null;
  arrived: boolean;
};

type CursorPointPayload = {
  x: number;
  y: number;
  label: string | null;
  screen: number | null;
};

// Quadratic bezier point: P(t) = (1-t)²·P0 + 2(1-t)t·P1 + t²·P2
function bezier(t: number, p0: number, p1: number, p2: number): number {
  const mt = 1 - t;
  return mt * mt * p0 + 2 * mt * t * p1 + t * t * p2;
}

const FLIGHT_DURATION = 380; // ms
const LINGER_DURATION = 1600; // ms after landing before hiding
const INDICATOR_SIZE = 56; // px — half-size for centering
const LABEL_HEIGHT = 32; // extra window height when showing label

const DesktopCursorOverlay = () => {
  const [cursorHighlight, setCursorHighlight] =
    useState<CursorHighlight | null>(null);
  const [clickVisualizations, setClickVisualizations] = useState<
    ClickVisualization[]
  >([]);
  const [flyingCursor, setFlyingCursor] = useState<FlyingCursor | null>(null);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem("juno-show-desktop-cursor-visualization") !== "false"
  );

  const overlayWindowRef = useRef<WebviewWindow | null>(null);
  const lastPositionUpdate = useRef<number>(0);
  const positionUpdateThrottle = 16;
  const flyAnimFrameRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flyLingerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Remember last landed position so next flight can depart from there
  const lastLandedRef = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    const initializeOverlay = async () => {
      let window = await WebviewWindow.getByLabel("desktop-cursor-overlay");

      if (!window) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("open_desktop_cursor_overlay");
          window = await WebviewWindow.getByLabel("desktop-cursor-overlay");
        } catch (error) {
          console.error(
            "Failed to create desktop cursor overlay window:",
            error
          );
          return;
        }
      }

      overlayWindowRef.current = window;
    };

    initializeOverlay();

    return () => {
      if (flyAnimFrameRef.current) clearTimeout(flyAnimFrameRef.current);
      if (flyLingerRef.current) clearTimeout(flyLingerRef.current);
    };
  }, []);

  useEffect(() => {
    const checkSettings = () => {
      const enabled =
        localStorage.getItem("juno-show-desktop-cursor-visualization") !==
        "false";
      setIsEnabled(enabled);
    };

    checkSettings();
    const interval = setInterval(checkSettings, 1000);
    return () => clearInterval(interval);
  }, []);

  const positionOverlay = async (
    x: number,
    y: number,
    force: boolean = false
  ) => {
    if (!overlayWindowRef.current) return;

    const now = Date.now();
    if (!force && now - lastPositionUpdate.current < positionUpdateThrottle) {
      return;
    }
    lastPositionUpdate.current = now;

    try {
      const offsetX = Math.max(0, x - 100);
      const offsetY = Math.max(0, y - 100);

      await overlayWindowRef.current.setPosition(
        new LogicalPosition(offsetX, offsetY)
      );

      await overlayWindowRef.current.setSize(new LogicalSize(200, 200));
    } catch (error) {
      console.warn("Failed to position overlay window:", error);
    }
  };

  // Fly the overlay window from (startX,startY) to (targetX,targetY) along a bezier arc
  const flyTo = useCallback(
    async (targetX: number, targetY: number, label: string | null) => {
      if (!overlayWindowRef.current) return;

      // Cancel any previous flight
      if (flyAnimFrameRef.current) clearTimeout(flyAnimFrameRef.current);
      if (flyLingerRef.current) clearTimeout(flyLingerRef.current);

      const startX = lastLandedRef.current?.x ?? targetX - 250;
      const startY = lastLandedRef.current?.y ?? targetY - 80;

      // Control point: perpendicular offset from the midpoint of straight path
      const midX = (startX + targetX) / 2;
      const midY = (startY + targetY) / 2;
      const dx = targetX - startX;
      const dy = targetY - startY;
      // Perpendicular offset = 15% of distance, capped so it feels natural
      const dist = Math.sqrt(dx * dx + dy * dy);
      const perpScale = Math.min(0.15, 80 / Math.max(dist, 1));
      const ctrlX = midX - dy * perpScale;
      const ctrlY = midY + dx * perpScale;

      const windowW = INDICATOR_SIZE * 2;
      const windowH = INDICATOR_SIZE * 2 + (label ? LABEL_HEIGHT : 0);

      setFlyingCursor({ label, arrived: false });

      // Show the window at start position
      try {
        await overlayWindowRef.current.setSize(new LogicalSize(windowW, windowH));
        await overlayWindowRef.current.setPosition(
          new LogicalPosition(startX - INDICATOR_SIZE, startY - INDICATOR_SIZE)
        );
        await overlayWindowRef.current.show();
      } catch (err) {
        console.warn("Failed to show overlay for flight:", err);
      }

      const startTime = Date.now();

      const tick = async () => {
        const elapsed = Date.now() - startTime;
        const tRaw = Math.min(elapsed / FLIGHT_DURATION, 1);

        // Ease-in-out cubic
        const t =
          tRaw < 0.5
            ? 4 * tRaw * tRaw * tRaw
            : 1 - Math.pow(-2 * tRaw + 2, 3) / 2;

        const curX = bezier(t, startX, ctrlX, targetX);
        const curY = bezier(t, startY, ctrlY, targetY);

        try {
          await overlayWindowRef.current?.setPosition(
            new LogicalPosition(curX - INDICATOR_SIZE, curY - INDICATOR_SIZE)
          );
        } catch {
          // Window may have been hidden — abort
          return;
        }

        if (tRaw < 1) {
          flyAnimFrameRef.current = setTimeout(tick, 16);
        } else {
          // Landed
          lastLandedRef.current = { x: targetX, y: targetY };
          setFlyingCursor({ label, arrived: true });

          // Expand window if we have a label to display
          if (label) {
            try {
              await overlayWindowRef.current?.setSize(
                new LogicalSize(Math.max(windowW, label.length * 9 + 32), windowH)
              );
            } catch {
              // ignore
            }
          }

          flyLingerRef.current = setTimeout(async () => {
            setFlyingCursor(null);
            try {
              await overlayWindowRef.current?.hide();
            } catch {
              // ignore
            }
          }, LINGER_DURATION);
        }
      };

      flyAnimFrameRef.current = setTimeout(tick, 0);
    },
    []
  );

  // Listen for agent [POINT:x,y:label:screenN] events
  useEventListener<CursorPointPayload>(EVENTS.UI_CURSOR_POINT, (payload) => {
    if (!isEnabled) return;
    flyTo(payload.x, payload.y, payload.label ?? null);
  });

  // Listen for cursor highlight events (manual/computer-use flow)
  useEffect(() => {
    if (!isEnabled) return;

    let mounted = true;
    const unlistenFns: (() => void)[] = [];

    const setupListeners = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");

        const unlistenStart = await listen<[number, number]>(
          "ui-cursor-highlight-start",
          async (event) => {
            if (!mounted) return;
            const [x, y] = event.payload;
            setCursorHighlight({ x, y, active: true, timestamp: Date.now() });
            await positionOverlay(x, y, true);
            if (overlayWindowRef.current) {
              await overlayWindowRef.current.show();
            }
          }
        );
        if (mounted) unlistenFns.push(unlistenStart);
        else unlistenStart();

        const unlistenMove = await listen<[number, number]>(
          "ui-cursor-highlight-move",
          async (event) => {
            if (!mounted) return;
            const [x, y] = event.payload;
            setCursorHighlight((prev) =>
              prev ? { ...prev, x, y, timestamp: Date.now() } : null
            );
            await positionOverlay(x, y);
          }
        );
        if (mounted) unlistenFns.push(unlistenMove);
        else unlistenMove();

        const unlistenStop = await listen<[number, number]>(
          "ui-cursor-highlight-stop",
          async () => {
            if (!mounted) return;
            setCursorHighlight(null);
            setTimeout(async () => {
              if (mounted && overlayWindowRef.current) {
                await overlayWindowRef.current.hide();
              }
            }, 500);
          }
        );
        if (mounted) unlistenFns.push(unlistenStop);
        else unlistenStop();

        const unlistenClick = await listen<[number, number, string]>(
          "click-visualization",
          async (event) => {
            if (!mounted) return;
            const [x, y, color] = event.payload;
            const newClick: ClickVisualization = {
              x,
              y,
              color,
              id: Date.now(),
              timestamp: Date.now(),
            };

            setClickVisualizations((prev) => [...prev, newClick]);
            await positionOverlay(x, y, true);
            if (overlayWindowRef.current) {
              await overlayWindowRef.current.show();
            }
            setTimeout(async () => {
              if (mounted && overlayWindowRef.current) {
                await overlayWindowRef.current.hide();
              }
            }, 1000);
          }
        );
        if (mounted) unlistenFns.push(unlistenClick);
        else unlistenClick();
      } catch (error) {
        console.error("Failed to setup cursor overlay listeners:", error);
      }
    };

    setupListeners();

    return () => {
      mounted = false;
      for (const unlisten of unlistenFns) {
        unlisten();
      }
    };
  }, [isEnabled]);

  useEffect(() => {
    if (clickVisualizations.length === 0) return;

    const cleanupTimeout = setTimeout(() => {
      const now = Date.now();
      setClickVisualizations((prev) =>
        prev.filter((click) => now - click.timestamp < 1500)
      );
    }, 100);

    return () => clearTimeout(cleanupTimeout);
  }, [clickVisualizations]);

  if (!isEnabled) {
    return null;
  }

  return (
    <div
      className="desktop-cursor-overlay"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "200px",
        height: "200px",
        pointerEvents: "none",
        zIndex: 999999,
        overflow: "visible",
      }}
    >
      {/* Flying cursor — shown during agent [POINT] animation */}
      {flyingCursor && (
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: "translate(-50%, -50%)",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: "6px",
          }}
        >
          {/* Cursor arrow SVG */}
          <div
            style={{
              animation: flyingCursor.arrived
                ? "point-land 0.25s ease-out forwards"
                : "point-fly 0.38s ease-out forwards",
            }}
          >
            <svg
              width="36"
              height="36"
              viewBox="0 0 36 36"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              {/* Drop shadow */}
              <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
                <feDropShadow
                  dx="1"
                  dy="2"
                  stdDeviation="2"
                  floodColor="rgba(0,0,0,0.4)"
                />
              </filter>
              {/* Cursor arrow */}
              <path
                d="M6 4L28 18L17 19.5L12 30L6 4Z"
                fill="white"
                stroke="#1a1a2e"
                strokeWidth="2"
                strokeLinejoin="round"
                filter="url(#shadow)"
              />
              {/* Accent dot at tip */}
              <circle cx="6.5" cy="4.5" r="2.5" fill="#6366f1" />
            </svg>
          </div>

          {/* Label shown on arrival */}
          {flyingCursor.arrived && flyingCursor.label && (
            <div
              style={{
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
                animation: "label-appear 0.18s ease-out forwards",
                boxShadow: "0 2px 12px rgba(0,0,0,0.3)",
              }}
            >
              {flyingCursor.label}
            </div>
          )}

          {/* Landing ripple */}
          {flyingCursor.arrived && (
            <div
              style={{
                position: "absolute",
                left: "50%",
                top: "12px",
                width: "40px",
                height: "40px",
                transform: "translate(-50%, -50%)",
                borderRadius: "50%",
                border: "2px solid rgba(99,102,241,0.6)",
                animation: "point-ripple 0.6s ease-out forwards",
                pointerEvents: "none",
              }}
            />
          )}
        </div>
      )}

      {/* Cursor highlight circle — computer-use mouse tracking */}
      {cursorHighlight && cursorHighlight.active && (
        <div
          className="cursor-highlight-circle"
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "60px",
            height: "60px",
            borderRadius: "50%",
            border: "3px solid rgba(74, 144, 226, 0.8)",
            backgroundColor: "rgba(74, 144, 226, 0.1)",
            transform: "translate(-50%, -50%)",
            animation: "cursor-pulse 1.5s ease-in-out infinite",
            boxShadow:
              "0 0 20px rgba(74, 144, 226, 0.4), inset 0 0 20px rgba(74, 144, 226, 0.1)",
          }}
        />
      )}

      {cursorHighlight && cursorHighlight.active && (
        <div
          className="cursor-ripple"
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "100px",
            height: "100px",
            borderRadius: "50%",
            border: "2px solid rgba(74, 144, 226, 0.3)",
            transform: "translate(-50%, -50%)",
            animation: "cursor-ripple 2s ease-out infinite",
          }}
        />
      )}

      {/* Click visualizations */}
      {clickVisualizations.map((click) => (
        <div
          key={click.id}
          className="desktop-click-indicator"
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "40px",
            height: "40px",
            borderRadius: "50%",
            backgroundColor: `${click.color}60`,
            border: `3px solid ${click.color}`,
            transform: "translate(-50%, -50%)",
            animation: "desktop-click-animation 1s ease-out forwards",
            boxShadow: `0 0 15px ${click.color}40`,
          }}
        />
      ))}

      <style>{`
        @keyframes point-fly {
          0% { opacity: 0; transform: scale(0.6) translateY(-6px); }
          60% { opacity: 1; transform: scale(1.35) translateY(0); }
          100% { opacity: 1; transform: scale(1) translateY(0); }
        }

        @keyframes point-land {
          0% { transform: scale(1.1); }
          50% { transform: scale(0.88); }
          100% { transform: scale(1); }
        }

        @keyframes point-ripple {
          0% { transform: translate(-50%, -50%) scale(0.4); opacity: 0.8; }
          100% { transform: translate(-50%, -50%) scale(2.4); opacity: 0; }
        }

        @keyframes label-appear {
          0% { opacity: 0; transform: translateY(4px); }
          100% { opacity: 1; transform: translateY(0); }
        }

        @keyframes cursor-pulse {
          0% { transform: translate(-50%, -50%) scale(1); opacity: 0.8; }
          50% { transform: translate(-50%, -50%) scale(1.1); opacity: 1; }
          100% { transform: translate(-50%, -50%) scale(1); opacity: 0.8; }
        }

        @keyframes cursor-ripple {
          0% { transform: translate(-50%, -50%) scale(0.8); opacity: 0.6; }
          100% { transform: translate(-50%, -50%) scale(1.8); opacity: 0; }
        }

        @keyframes desktop-click-animation {
          0% { transform: translate(-50%, -50%) scale(0.3); opacity: 1; }
          50% { transform: translate(-50%, -50%) scale(1.2); opacity: 0.8; }
          100% { transform: translate(-50%, -50%) scale(2); opacity: 0; }
        }
      `}</style>
    </div>
  );
};

export default DesktopCursorOverlay;
