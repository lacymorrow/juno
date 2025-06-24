import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, useRef } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

// Cursor highlight and click visualization types
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

const DesktopCursorOverlay = () => {
  const [cursorHighlight, setCursorHighlight] =
    useState<CursorHighlight | null>(null);
  const [clickVisualizations, setClickVisualizations] = useState<
    ClickVisualization[]
  >([]);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem("juno-show-desktop-cursor-visualization") !== "false"
  );
  const overlayWindowRef = useRef<any>(null);

  // Initialize overlay window reference and create window if needed
  useEffect(() => {
    const initializeOverlay = async () => {
      // Try to get existing window
      let window = WebviewWindow.getByLabel("desktop-cursor-overlay");

      if (!window) {
        // Window doesn't exist, create it
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("open_desktop_cursor_overlay");
          // Get the window reference after creation
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

  // Check localStorage for settings changes
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

  // Position overlay window at cursor location
  const positionOverlay = async (x: number, y: number) => {
    if (!overlayWindowRef.current) return;

    try {
      // Position the window slightly offset from cursor to center the visualization
      const offsetX = Math.max(0, x - 100); // Center 200px visualization circle
      const offsetY = Math.max(0, y - 100);

      await overlayWindowRef.current.setPosition({
        type: "Physical",
        x: offsetX,
        y: offsetY,
      });

      // Resize window to accommodate visualization (200x200 for circles)
      await overlayWindowRef.current.setSize({
        type: "Physical",
        width: 200,
        height: 200,
      });
    } catch (error) {
      console.warn("Failed to position overlay window:", error);
    }
  };

  // Listen for cursor highlight events
  useEffect(() => {
    if (!isEnabled) return;

    const setupListeners = async () => {
      // Cursor highlight start
      const unlistenStart = await listen<[number, number]>(
        "ui-cursor-highlight-start",
        async (event) => {
          const [x, y] = event.payload;
          setCursorHighlight({ x, y, active: true, timestamp: Date.now() });
          await positionOverlay(x, y);

          // Show overlay window
          if (overlayWindowRef.current) {
            await overlayWindowRef.current.show();
          }
        }
      );

      // Cursor highlight move
      const unlistenMove = await listen<[number, number]>(
        "ui-cursor-highlight-move",
        async (event) => {
          const [x, y] = event.payload;
          setCursorHighlight((prev) =>
            prev ? { ...prev, x, y, timestamp: Date.now() } : null
          );
          await positionOverlay(x, y);
        }
      );

      // Cursor highlight stop
      const unlistenStop = await listen<[number, number]>(
        "ui-cursor-highlight-stop",
        async () => {
          setCursorHighlight(null);

          // Hide overlay window after brief delay
          setTimeout(async () => {
            if (overlayWindowRef.current) {
              await overlayWindowRef.current.hide();
            }
          }, 500);
        }
      );

      // Click visualizations
      const unlistenClick = await listen<[number, number, string]>(
        "click-visualization",
        async (event) => {
          const [x, y, color] = event.payload;
          const newClick: ClickVisualization = {
            x,
            y,
            color,
            id: Date.now(),
            timestamp: Date.now(),
          };

          setClickVisualizations((prev) => [...prev, newClick]);
          await positionOverlay(x, y);

          // Show overlay window for click
          if (overlayWindowRef.current) {
            await overlayWindowRef.current.show();
          }

          // Hide after animation
          setTimeout(async () => {
            if (overlayWindowRef.current) {
              await overlayWindowRef.current.hide();
            }
          }, 1000);
        }
      );

      return () => {
        unlistenStart();
        unlistenMove();
        unlistenStop();
        unlistenClick();
      };
    };

    setupListeners();
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

  // Don't render if disabled
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
      {/* Cursor highlight circle - smooth movement indicator */}
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

      {/* Additional ripple effect for movement */}
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

      {/* Styles for animations */}
      <style>{`
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
