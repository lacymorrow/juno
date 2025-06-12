import TransparentFloatingPanel from "@/components/TransparentFloatingPanel";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import "./styles/globals.css";

export default function FloatingPanel() {
  useEffect(() => {
    const setupWindow = async () => {
      const appWindow = getCurrentWindow();

      // Set up window properties
      await appWindow.setAlwaysOnTop(true);
      await appWindow.setSkipTaskbar(true);
      await appWindow.setResizable(false);
      await appWindow.setTitle("");

      // Enable mouse events but allow click-through when not hovering
      await appWindow.setIgnoreCursorEvents(false);
    };

    setupWindow();
  }, []);

  return (
    <div className="w-screen h-screen bg-transparent overflow-hidden pointer-events-none">
      <TransparentFloatingPanel />
    </div>
  );
}
