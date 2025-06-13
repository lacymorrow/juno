import TransparentFloatingPanel from "@/components/TransparentFloatingPanel";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./styles/globals.css";

export default function FloatingPanel() {
  const [isHovered, setIsHovered] = useState(false);
  const [windowReady, setWindowReady] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    const element = contentRef.current;
    if (!element || !windowReady) return;

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;

        const paddedWidth = Math.ceil(width) + 2;
        const paddedHeight = Math.ceil(height) + 2;

        console.log(
          `Content resized: ${width}x${height}. Resizing window to ${paddedWidth}x${paddedHeight}`
        );
        invoke("update_floating_panel_size", {
          width: paddedWidth,
          height: paddedHeight,
        }).catch(console.error);
      }
    });

    // Observe the direct child of the ref'd div, which should be the content container.
    // This avoids observing the parent which is stuck at 100% width/height.
    if (element.children[0]) {
      resizeObserver.observe(element.children[0]);
    } else {
      console.warn(
        "ResizeObserver: Could not find a child element to observe."
      );
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [windowReady]);

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
          <div ref={contentRef} className="inline-block">
            <TransparentFloatingPanel
              isWindowHovered={isHovered}
              className="h-full w-full"
            />
          </div>
        </div>
      )}
    </div>
  );
}
