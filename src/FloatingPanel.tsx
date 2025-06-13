import TransparentFloatingPanel from "@/components/TransparentFloatingPanel";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import "./styles/globals.css";

export default function FloatingPanel() {
  const [isHovered, setIsHovered] = useState(false);
  const [windowReady, setWindowReady] = useState(false);

  useEffect(() => {
    const setupWindow = async () => {
      const appWindow = getCurrentWindow();

      try {
        // Set up core window properties
        await appWindow.setAlwaysOnTop(true);
        await appWindow.setSkipTaskbar(true);
        await appWindow.setResizable(false);
        await appWindow.setTitle("");

        // Configure for click-through behavior
        // Start with mouse events enabled - the component will handle pointer-events CSS
        await appWindow.setIgnoreCursorEvents(false);

        // Set up window event listeners for enhanced interaction
        const unlistenFocus = await appWindow.onFocusChanged(
          ({ payload: focused }) => {
            console.log("Floating panel focus changed:", focused);
          }
        );

        setWindowReady(true);

        // Cleanup function
        return () => {
          unlistenFocus();
        };
      } catch (error) {
        console.error("Failed to setup floating panel window:", error);
        setWindowReady(true); // Still show the panel even if setup fails
      }
    };

    setupWindow();
  }, []);

  // Listen for Rust-based window hover events (same as floating bar)
  useEffect(() => {
    let unlistenEnter: (() => void) | undefined;
    let unlistenLeave: (() => void) | undefined;

    const setupListeners = async () => {
      unlistenEnter = await listen<null>("mouse-entered-window", () => {
        setIsHovered(true);
      });

      unlistenLeave = await listen<null>("mouse-left-window", () => {
        setIsHovered(false);
      });
    };

    setupListeners();
    return () => {
      unlistenEnter?.();
      unlistenLeave?.();
    };
  }, []);

  return (
    <div
      className="w-screen h-screen bg-transparent overflow-hidden"
      style={{
        // Enable pointer events for the entire window area
        pointerEvents: "auto",
        // Add 3D perspective for the shelf effect
        perspective: "1200px",
        perspectiveOrigin: "center bottom",
      }}
    >
      {windowReady && (
        <div
          className="h-full w-full transition-all duration-700 ease-out"
          style={{
            transform: isHovered
              ? "rotateX(0deg) translateY(0px) translateZ(0px) scale(1)"
              : "rotateX(-12deg) translateY(-8px) translateZ(-50px) scale(0.95)",
            transformOrigin: "center bottom",
            opacity: isHovered ? 1 : 0.85,
            filter: isHovered ? "blur(0px)" : "blur(0.5px)",
          }}
        >
          <TransparentFloatingPanel isWindowHovered={isHovered} />
        </div>
      )}
    </div>
  );
}
