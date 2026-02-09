import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

type KeyPressInfo = {
  key: string;
  modifier?: string;
  id: number;
  timestamp: number;
};

const KeyPressOverlay = () => {
  const [keyPresses, setKeyPresses] = useState<KeyPressInfo[]>([]);
  const [isEnabled, setIsEnabled] = useState(
    localStorage.getItem('juno-show-key-press-overlay') === 'true'
  );

  // Check localStorage periodically for setting changes
  useEffect(() => {
    const checkSettings = () => {
      const enabled = localStorage.getItem('juno-show-key-press-overlay') === 'true';
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

    let unlisten: (() => void) | undefined;
    let mounted = true;

    listen<{ key: string; modifier?: string }>(
      "key-press-visualization",
      (event) => {
        if (!mounted) return;
        const { key, modifier } = event.payload;
        const newKeyPress: KeyPressInfo = {
          key,
          modifier,
          id: Date.now() + Math.random(),
          timestamp: Date.now(),
        };
        setKeyPresses((prevKeyPresses) => [...prevKeyPresses, newKeyPress]);
      }
    ).then((fn) => {
      if (mounted) {
        unlisten = fn;
      } else {
        safeCleanupEventListener(fn);
      }
    }).catch((err) => console.error("Failed to setup key-press listener:", err));

    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, [isEnabled]);

  // Clean up old key presses (remove after animation duration)
  useEffect(() => {
    if (keyPresses.length === 0 || !isEnabled) return;

    const cleanupTimeout = setTimeout(() => {
      const now = Date.now();
      setKeyPresses((prevKeyPresses) =>
        prevKeyPresses.filter((keyPress) => now - keyPress.timestamp < 2000)
      ); // Remove key presses older than 2 seconds
    }, 100); // Check more frequently for smoother cleanup

    return () => clearTimeout(cleanupTimeout);
  }, [keyPresses, isEnabled]);

  // Don't render anything if disabled
  if (!isEnabled) {
    return null;
  }

  // Format key display text
  const formatKeyDisplay = (keyPress: KeyPressInfo) => {
    if (keyPress.modifier) {
      return `${keyPress.modifier}+${keyPress.key}`;
    }
    return keyPress.key;
  };

  return (
    <div
      className="key-press-overlay"
      style={{
        position: "fixed",
        top: "20px",
        right: "20px",
        pointerEvents: "none", // Allow clicks to pass through
        zIndex: 999998, // High z-index but below click visualizer
        display: "flex",
        flexDirection: "column",
        gap: "4px",
        maxWidth: "200px",
      }}
    >
      {keyPresses.map((keyPress, index) => (
        <div
          key={keyPress.id}
          className="key-press-indicator"
          style={{
            backgroundColor: "rgba(0, 0, 0, 0.8)",
            color: "white",
            padding: "6px 12px",
            borderRadius: "6px",
            fontSize: "14px",
            fontFamily: "monospace",
            border: "1px solid rgba(255, 255, 255, 0.2)",
            animation: `key-press-animation 2s ease-out forwards`,
            animationDelay: `${index * 50}ms`, // Stagger animations for multiple rapid key presses
            backdropFilter: "blur(4px)",
            transform: "translateX(0)", // Start position for slide-in animation
          }}
        >
          <kbd style={{ 
            backgroundColor: "rgba(255, 255, 255, 0.1)",
            padding: "2px 6px",
            borderRadius: "3px",
            fontSize: "12px",
            border: "1px solid rgba(255, 255, 255, 0.2)"
          }}>
            {formatKeyDisplay(keyPress)}
          </kbd>
        </div>
      ))}
      <style>{`
        @keyframes key-press-animation {
          0% {
            opacity: 0;
            transform: translateX(100px);
          }
          10% {
            opacity: 1;
            transform: translateX(0);
          }
          90% {
            opacity: 1;
            transform: translateX(0);
          }
          100% {
            opacity: 0;
            transform: translateX(-20px);
          }
        }
      `}</style>
    </div>
  );
};

export default KeyPressOverlay;