import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PillDemoProps {
  onClose?: () => void;
}

export default function PillDemoPanel({ onClose }: PillDemoProps) {
  const [activeDemo, setActiveDemo] = useState<string>("basic");

  const createNewPillWindow = useCallback(async (variant: string) => {
    try {
      const configs = {
        basic: { width: 200, height: 50 },
        small: { width: 120, height: 40 },
        large: { width: 300, height: 60 },
        wide: { width: 400, height: 50 },
      };

      const config = configs[variant as keyof typeof configs] || configs.basic;

      await invoke("create_pill_window", {
        label: `pill-${variant}-${Date.now()}`,
        width: config.width,
        height: config.height,
        x: Math.random() * 200 + 100,
        y: Math.random() * 200 + 100,
      });
      await invoke("apply_pill_vibrancy", { windowLabel: "floating-panel" });
    } catch (error) {
      console.error("Failed to create pill window:", error);
    }
  }, []);

  const applyVibrancy = useCallback(async () => {
    try {
      await invoke("apply_pill_vibrancy", { windowLabel: "floating-panel" });
    } catch (error) {
      console.error("Failed to apply vibrancy:", error);
    }
  }, []);

  const removeVibrancy = useCallback(async () => {
    try {
      await invoke("remove_vibrancy", { windowLabel: "floating-panel" });
    } catch (error) {
      console.error("Failed to remove vibrancy:", error);
    }
  }, []);

  const makeInteractive = useCallback(async () => {
    try {
      await invoke("make_window_interactive", {
        windowLabel: "floating-panel",
      });
    } catch (error) {
      console.error("Failed to make window interactive:", error);
    }
  }, []);

  applyVibrancy();

  return (
    <div className="p-4 space-y-4 max-w-md">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-gray-600 pb-2">
        <h3 className="text-lg font-medium text-white">Pill Demo</h3>
        {onClose && (
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-300 transition-colors"
          >
            ✕
          </button>
        )}
      </div>

      {/* Demo Pills */}
      <div className="space-y-4">
        <div>
          <h4 className="text-sm font-medium text-gray-300 mb-2">
            CSS-Only Pills
          </h4>
          <div className="space-y-2">
            {/* Basic pill */}
            <div className="pill-container pill-medium pill-medium-blur">
              <div className="pill-content">
                <span className="pill-text">Basic Pill</span>
              </div>
            </div>

            {/* Light blur pill */}
            <div className="pill-container pill-medium pill-light-blur">
              <div className="pill-content">
                <span className="pill-text">Light Blur</span>
              </div>
            </div>

            {/* Heavy blur pill */}
            <div className="pill-container pill-medium pill-heavy-blur">
              <div className="pill-content">
                <span className="pill-text">Heavy Blur</span>
              </div>
            </div>

            {/* Animated pill */}
            <div className="pill-container pill-medium pill-medium-blur pill-animated">
              <div className="pill-content">
                <span className="pill-text">Animated</span>
              </div>
            </div>

            {/* Colored pills */}
            <div className="pill-container pill-medium pill-blue">
              <div className="pill-content">
                <span className="pill-text">Blue Theme</span>
              </div>
            </div>

            <div className="pill-container pill-medium pill-purple">
              <div className="pill-content">
                <span className="pill-text">Purple Theme</span>
              </div>
            </div>

            <div className="pill-container pill-medium pill-green">
              <div className="pill-content">
                <span className="pill-text">Green Theme</span>
              </div>
            </div>
          </div>
        </div>

        {/* Native Vibrancy Controls */}
        <div>
          <h4 className="text-sm font-medium text-gray-300 mb-2">
            Native Vibrancy
          </h4>
          <div className="space-y-2">
            <button
              onClick={applyVibrancy}
              className="w-full px-3 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
            >
              Apply Native Vibrancy
            </button>
            <button
              onClick={removeVibrancy}
              className="w-full px-3 py-2 bg-gray-600 hover:bg-gray-700 text-white rounded-lg transition-colors"
            >
              Remove Vibrancy
            </button>
            <button
              onClick={makeInteractive}
              className="w-full px-3 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-lg transition-colors"
            >
              Make Interactive (Not Click-Through)
            </button>
          </div>
        </div>

        {/* Create New Windows */}
        <div>
          <h4 className="text-sm font-medium text-gray-300 mb-2">
            Create New Pill Windows
          </h4>
          <div className="grid grid-cols-2 gap-2">
            <button
              onClick={() => createNewPillWindow("small")}
              className="px-3 py-2 bg-green-600 hover:bg-green-700 text-white rounded text-sm transition-colors"
            >
              Small Pill
            </button>
            <button
              onClick={() => createNewPillWindow("basic")}
              className="px-3 py-2 bg-green-600 hover:bg-green-700 text-white rounded text-sm transition-colors"
            >
              Basic Pill
            </button>
            <button
              onClick={() => createNewPillWindow("large")}
              className="px-3 py-2 bg-green-600 hover:bg-green-700 text-white rounded text-sm transition-colors"
            >
              Large Pill
            </button>
            <button
              onClick={() => createNewPillWindow("wide")}
              className="px-3 py-2 bg-green-600 hover:bg-green-700 text-white rounded text-sm transition-colors"
            >
              Wide Pill
            </button>
          </div>
        </div>

        {/* Size comparison */}
        <div>
          <h4 className="text-sm font-medium text-gray-300 mb-2">
            Size Variants
          </h4>
          <div className="space-y-2">
            <div className="pill-container pill-small pill-medium-blur">
              <div className="pill-content">
                <span className="pill-text">Small</span>
              </div>
            </div>
            <div className="pill-container pill-medium pill-medium-blur">
              <div className="pill-content">
                <span className="pill-text">Medium</span>
              </div>
            </div>
            <div className="pill-container pill-large pill-medium-blur">
              <div className="pill-content">
                <span className="pill-text">Large</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Instructions */}
      <div className="text-xs text-gray-400 border-t border-gray-600 pt-3">
        <p>• CSS pills use backdrop-filter for cross-platform compatibility</p>
        <p>• Native vibrancy uses macOS-specific window effects</p>
        <p>• New windows demonstrate programmatic pill creation</p>
      </div>
    </div>
  );
}
