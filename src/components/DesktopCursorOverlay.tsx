import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, useRef } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

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

type ComputerUsePreview = {
  x: number;
  y: number;
  action: string;
  id: number;
};

const ACTION_LABELS: Record<string, string> = {
  left_click: "clicking",
  right_click: "right-clicking",
  middle_click: "middle-clicking",
  double_click: "double-clicking",
  triple_click: "triple-clicking",
  left_click_drag: "dragging",
  mouse_move: "moving to",
  left_mouse_down: "pressing",
  left_mouse_up: "releasing",
  scroll: "scrolling",
};

const DesktopCursorOverlay = () => {
  const [cursorHighlight, setCursorHighlight] =
    useState<CursorHighlight | null>(null);
  const [clickVisualizations, setClickVisualizations] = useState<
    ClickVisualization[]
  >([]);
  const [computerUsePreview, setComputerUsePreview] =
    useState<ComputerUsePreview | null>(null);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem("juno-show-desktop-cursor-visualization") !== "false"
  );
  const overlayWindowRef = useRef<any>(null);
  const previewDismissRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const initializeOverlay = async () => {
      let window = WebviewWindow.getByLabel("desktop-cursor-overlay");

      if (!window) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("open_desktop_cursor_overlay");
          window = WebviewWindow.getByLabel("desktop-cursor-overlay");
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

  const lastPositionUpdate = useRef<number>(0);
  const positionUpdateThrottle = 16;

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

      await overlayWindowRef.current.setPosition({
        type: "Logical",
        x: offsetX,
        y: offsetY,
      });

      await overlayWindowRef.current.setSize({
        type: "Logical",
        width: 200,
        height: 200,
      });
    } catch (error) {
      console.warn("Failed to position overlay window:", error);
    }
  };

  useEffect(() => {
    if (!isEnabled) return;

    let mounted = true;
    const unlistenFns: (() => void)[] = [];

    const setupListeners = async () => {
      try {
        // Cursor highlight start
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

        // Cursor highlight move
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

        // Cursor highlight stop
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

        // Click visualizations (post-action)
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

        // Computer use preview (pre-action) — shows targeting highlight BEFORE the action fires
        const unlistenPreview = await listen<{
          action: string;
          coordinate: [number, number];
          timestamp: number;
        }>("computer-use-preview", async (event) => {
          if (!mounted) return;
          const { action, coordinate } = event.payload;
          const [x, y] = coordinate;

          // Cancel any pending auto-dismiss from a previous preview
          if (previewDismissRef.current !== null) {
            clearTimeout(previewDismissRef.current);
            previewDismissRef.current = null;
          }

          setComputerUsePreview({ x, y, action, id: Date.now() });
          await positionOverlay(x, y, true);
          if (overlayWindowRef.current) {
            await overlayWindowRef.current.show();
          }

          // Auto-dismiss after 500ms — matches action cooldown with a small buffer
          previewDismissRef.current = setTimeout(async () => {
            if (!mounted) return;
            setComputerUsePreview(null);
            previewDismissRef.current = null;
          }, 500);
        });
        if (mounted) unlistenFns.push(unlistenPreview);
        else unlistenPreview();
      } catch (error) {
        console.error("Failed to setup cursor overlay listeners:", error);
      }
    };

    setupListeners();

    return () => {
      mounted = false;
      if (previewDismissRef.current !== null) {
        clearTimeout(previewDismissRef.current);
      }
      for (const unlisten of unlistenFns) {
        unlisten();
      }
    };
  }, [isEnabled]);

  // Clean up old click visualizations
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

  const previewLabel = computerUsePreview
    ? ACTION_LABELS[computerUsePreview.action] ?? computerUsePreview.action
    : null;

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
      {/* Pre-action targeting highlight — appears BEFORE the agent acts */}
      {computerUsePreview && (
        <>
          {/* Outer expanding ring */}
          <div
            key={`preview-outer-${computerUsePreview.id}`}
            style={{
              position: "absolute",
              left: "50%",
              top: "50%",
              width: "80px",
              height: "80px",
              borderRadius: "50%",
              border: "2px solid rgba(99, 179, 237, 0.5)",
              transform: "translate(-50%, -50%)",
              animation: "preview-expand 0.5s ease-out forwards",
            }}
          />
          {/* Inner targeting reticle */}
          <div
            key={`preview-inner-${computerUsePreview.id}`}
            style={{
              position: "absolute",
              left: "50%",
              top: "50%",
              width: "36px",
              height: "36px",
              borderRadius: "50%",
              border: "2.5px solid rgba(66, 153, 225, 0.9)",
              backgroundColor: "rgba(66, 153, 225, 0.15)",
              transform: "translate(-50%, -50%)",
              animation: "preview-pulse 0.5s ease-out forwards",
              boxShadow:
                "0 0 12px rgba(66, 153, 225, 0.6), inset 0 0 8px rgba(66, 153, 225, 0.2)",
            }}
          />
          {/* Crosshair center dot */}
          <div
            style={{
              position: "absolute",
              left: "50%",
              top: "50%",
              width: "6px",
              height: "6px",
              borderRadius: "50%",
              backgroundColor: "rgba(66, 153, 225, 0.9)",
              transform: "translate(-50%, -50%)",
              animation: "preview-dot 0.5s ease-out forwards",
            }}
          />
          {/* Action label */}
          {previewLabel && (
            <div
              style={{
                position: "absolute",
                left: "50%",
                top: "calc(50% - 38px)",
                transform: "translateX(-50%)",
                fontSize: "10px",
                fontWeight: 600,
                color: "rgba(66, 153, 225, 0.95)",
                whiteSpace: "nowrap",
                textShadow:
                  "0 1px 4px rgba(0,0,0,0.8), 0 0 8px rgba(0,0,0,0.6)",
                letterSpacing: "0.02em",
                animation: "preview-label 0.5s ease-out forwards",
                fontFamily: "-apple-system, BlinkMacSystemFont, sans-serif",
              }}
            >
              {previewLabel}
            </div>
          )}
        </>
      )}

      {/* Cursor highlight circle — continuous movement indicator */}
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

      {/* Additional ripple for cursor movement */}
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

      {/* Post-action click flash */}
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
        @keyframes preview-expand {
          0% { transform: translate(-50%, -50%) scale(0.4); opacity: 0.8; }
          60% { transform: translate(-50%, -50%) scale(1.1); opacity: 0.5; }
          100% { transform: translate(-50%, -50%) scale(1.4); opacity: 0; }
        }

        @keyframes preview-pulse {
          0% { transform: translate(-50%, -50%) scale(0.6); opacity: 0; }
          20% { transform: translate(-50%, -50%) scale(1.1); opacity: 1; }
          80% { transform: translate(-50%, -50%) scale(1.0); opacity: 0.9; }
          100% { transform: translate(-50%, -50%) scale(0.95); opacity: 0; }
        }

        @keyframes preview-dot {
          0% { opacity: 0; transform: translate(-50%, -50%) scale(0); }
          20% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
          80% { opacity: 1; }
          100% { opacity: 0; }
        }

        @keyframes preview-label {
          0% { opacity: 0; transform: translateX(-50%) translateY(4px); }
          20% { opacity: 1; transform: translateX(-50%) translateY(0); }
          70% { opacity: 1; }
          100% { opacity: 0; transform: translateX(-50%) translateY(-2px); }
        }

        @keyframes cursor-pulse {
          0% {
            transform: translate(-50%, -50%) scale(1);
            opacity: 0.8;
          }
          50% {
            transform: translate(-50%, -50%) scale(1.1);
            opacity: 1;
          }
          100% {
            transform: translate(-50%, -50%) scale(1);
            opacity: 0.8;
          }
        }

        @keyframes cursor-ripple {
          0% {
            transform: translate(-50%, -50%) scale(0.8);
            opacity: 0.6;
          }
          100% {
            transform: translate(-50%, -50%) scale(1.8);
            opacity: 0;
          }
        }

        @keyframes desktop-click-animation {
          0% {
            transform: translate(-50%, -50%) scale(0.3);
            opacity: 1;
          }
          50% {
            transform: translate(-50%, -50%) scale(1.2);
            opacity: 0.8;
          }
          100% {
            transform: translate(-50%, -50%) scale(2);
            opacity: 0;
          }
        }
      `}</style>
    </div>
  );
};

export default DesktopCursorOverlay;
