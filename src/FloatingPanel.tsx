import TransparentFloatingPanel from "@/components/TransparentFloatingPanel";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useDragWindow } from "@/hooks/useDragWindow";
import { UI } from "@/lib/constants.generated";
import "./styles/globals.css";

// Constants for all panel sizes - these should match the TransparentFloatingPanel component
// Adding 24px padding (12px on each side) for each dimension
const COMPACT_WIDTH = 140 + 24; // 164
const COMPACT_HEIGHT = 60 + 24; // 84
const EXPANDED_WIDTH = 300 + 24; // 324
const EXPANDED_HEIGHT = 100 + 24; // 124
const CHAT_WIDTH = 350 + 24; // 374
const CHAT_HEIGHT = 250 + 24; // 274
const SETTINGS_WIDTH = 280 + 24; // 304
const SETTINGS_HEIGHT = 180 + 24; // 204

// Helper function to get window dimensions for each panel mode
function getWindowDimensionsForMode(
  mode: "compact" | "expanded" | "chat" | "settings"
) {
  switch (mode) {
    case "compact":
      return { width: COMPACT_WIDTH, height: COMPACT_HEIGHT };
    case "expanded":
      return { width: EXPANDED_WIDTH, height: EXPANDED_HEIGHT };
    case "chat":
      return { width: CHAT_WIDTH, height: CHAT_HEIGHT };
    case "settings":
      return { width: SETTINGS_WIDTH, height: SETTINGS_HEIGHT };
    default:
      return { width: COMPACT_WIDTH, height: COMPACT_HEIGHT };
  }
}

export default function FloatingPanel() {
  const onDragMouseDown = useDragWindow();
  const [isHovered, setIsHovered] = useState(false);
  const [windowReady, setWindowReady] = useState(false);
  const [panelMode, setPanelMode] = useState<
    "compact" | "expanded" | "chat" | "settings"
  >("compact");

  useEffect(() => {
    let mounted = true;
    let unlistenFocus: (() => void) | undefined;

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
        const fn = await appWindow.onFocusChanged(
          ({ payload: focused }) => {
            if (!mounted) return;
            console.log("Floating panel focus changed:", focused);
          }
        );

        if (mounted) {
          unlistenFocus = fn;
        } else {
          fn(); // Unmounted during setup
          return;
        }

        setWindowReady(true);
      } catch (error) {
        console.error("Failed to setup floating panel window:", error);
        if (mounted) setWindowReady(true); // Still show the panel even if setup fails
      }
    };

    setupWindow();

    return () => {
      mounted = false;
      unlistenFocus?.();
    };
  }, []);

  // Update window size based on panel mode changes with delayed shrinking for smooth transitions
  useEffect(() => {
    let timeoutId: NodeJS.Timeout | null = null;

    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        const dimensions = getWindowDimensionsForMode(panelMode);

        if (panelMode === "compact") {
          // Compact state - delay to allow CSS transitions to complete
          // The TransparentFloatingPanel has 700ms transitions, so we wait 750ms
          timeoutId = setTimeout(async () => {
            try {
              await appWindow.setSize(
                new LogicalSize(dimensions.width, dimensions.height)
              );
              timeoutId = null;
            } catch (err) {
              console.error(
                "Failed to resize floating panel window (delayed):",
                err
              );
            }
          }, 750);
        } else {
          // Expanded/chat/settings states - resize immediately with mode-specific dimensions
          // Clear any pending shrink timeout when expanding
          if (timeoutId) {
            clearTimeout(timeoutId);
            timeoutId = null;
          }

          await appWindow.setSize(
            new LogicalSize(dimensions.width, dimensions.height)
          );
        }
      } catch (err) {
        console.error("Failed to resize floating panel window:", err);
      }
    };

    if (windowReady) {
      resizeWindow();
    }

    // Cleanup timeout on unmount or dependency change
    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [panelMode, windowReady]);

  // Listen for Rust-based window hover events (same as floating bar)
  useEffect(() => {
    let mounted = true;
    let unlistenEnter: (() => void) | undefined;
    let unlistenLeave: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        const fnEnter = await listen<null>("mouse-entered-window", () => {
          if (mounted) setIsHovered(true);
        });
        if (mounted) unlistenEnter = fnEnter;
        else { fnEnter(); return; }

        const fnLeave = await listen<null>("mouse-left-window", () => {
          if (mounted) setIsHovered(false);
        });
        if (mounted) unlistenLeave = fnLeave;
        else { fnLeave(); return; }
      } catch (error) {
        console.error("Failed to setup hover listeners:", error);
      }
    };

    setupListeners();
    return () => {
      mounted = false;
      unlistenEnter?.();
      unlistenLeave?.();
    };
  }, []);

  return (
    <div
      className="w-screen h-screen bg-transparent overflow-hidden cursor-grab active:cursor-grabbing"
      onMouseDown={onDragMouseDown}
      style={{
        // Enable pointer events for the entire window area
        pointerEvents: "auto",
        // Add 3D perspective for the shelf effect
        perspective: "100px",
        perspectiveOrigin: "center bottom",
      }}
    >
      {windowReady && (
        <div
          className="h-full w-full transition-all duration-700 ease-out"
          style={{
            transform: isHovered
              ? "rotateX(0deg) translateY(0px) translateZ(0px) scale(1)"
              : "rotateX(-30deg) translateZ(-50px) scale(0.95)",
            transformOrigin: "center bottom",
            opacity: isHovered ? 1 : 0.85,
          }}
        >
          <TransparentFloatingPanel
            isVisible={true}
            agentStatus={
              isHovered ? UI.AGENT_STATUS_LISTENING : UI.AGENT_STATUS_IDLE
            }
            message="Panel is ready"
            onModeChange={setPanelMode}
          />
        </div>
      )}
    </div>
  );
}
