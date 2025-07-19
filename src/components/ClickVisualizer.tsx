import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { safeUnlisten } from "@/lib/tauri-event-utils";

type ClickInfo = {
  x: number;
  y: number;
  color: string;
  id: number; // Unique ID for each click visualization
  timestamp: number;
};

const ClickVisualizer = () => {
  const [clicks, setClicks] = useState<ClickInfo[]>([]);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem("juno-show-click-visualization") !== "false", // Default to true
  );

  // Check localStorage periodically for setting changes
  useEffect(() => {
    const checkSettings = () => {
      const enabled =
        localStorage.getItem("juno-show-click-visualization") !== "false";
      setIsEnabled(enabled);
    };

    // Check immediately and then every second
    checkSettings();
    const interval = setInterval(checkSettings, 1000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    // Only listen for events if overlay is enabled
    if (!isEnabled) return;

    // Listen for click visualization events from the backend
    const unlisten = listen<[number, number, string]>(
      "click-visualization",
      (event) => {
        const [x, y, color] = event.payload;
        // Add new click with unique ID
        const newClick: ClickInfo = {
          x,
          y,
          color,
          id: Date.now(), // Use timestamp as a unique ID
          timestamp: Date.now(),
        };

        setClicks((prevClicks) => [...prevClicks, newClick]);
      },
    );

    return () => {
      // Cleanup listener when component unmounts or is disabled
      unlisten
        .then((unlistenFn) => {
          safeUnlisten(unlistenFn);
        })
        .catch((error) => {
          // Handle promise rejection during cleanup
          console.debug("Click visualizer listener cleanup error (safe to ignore):", error);
        });
    };
  }, [isEnabled]);

  // Clean up old clicks (remove after animation duration)
  useEffect(() => {
    if (clicks.length === 0 || !isEnabled) return;

    const cleanupTimeout = setTimeout(() => {
      const now = Date.now();
      setClicks((prevClicks) =>
        prevClicks.filter((click) => now - click.timestamp < 1000),
      ); // Remove clicks older than 1 second
    }, 100); // Check more frequently for smoother cleanup

    return () => clearTimeout(cleanupTimeout);
  }, [clicks, isEnabled]);

  // Don't render anything if disabled
  if (!isEnabled) {
    return null;
  }

  return (
    <div
      className="click-visualizer"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        pointerEvents: "none", // Allow clicks to pass through
        zIndex: 999999, // Increased z-index to ensure visibility above all other elements
        overflow: "hidden", // Prevent any overflow issues
      }}
    >
      {clicks.map((click) => (
        <div
          key={click.id}
          className="click-indicator"
          style={{
            position: "absolute",
            left: `${click.x}px`,
            top: `${click.y}px`,
            width: "20px",
            height: "20px",
            borderRadius: "50%",
            backgroundColor: `${click.color}50`, // Add 50% transparency
            border: `2px solid ${click.color}`,
            transform: "translate(-50%, -50%)", // Center at cursor position
            animation: "click-animation 1s ease-out forwards",
          }}
        />
      ))}
      <style>{`
        @keyframes click-animation {
          0% {
            opacity: 1;
            transform: translate(-50%, -50%) scale(0.5);
          }
          100% {
            opacity: 0;
            transform: translate(-50%, -50%) scale(1.5);
          }
        }
      `}</style>
    </div>
  );
};

export default ClickVisualizer;
