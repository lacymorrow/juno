import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useRef, useCallback } from "react";

type HeatmapPoint = {
  x: number;
  y: number;
  timestamp: number;
  intensity: number;
  event_type: string;
};

type HeatmapData = {
  points: HeatmapPoint[];
  grid_size: number;
  screen_width: number;
  screen_height: number;
  start_time: number;
  last_update: number;
};

const HeatmapVisualizer = () => {
  const [isTracking, setIsTracking] = useState(false);
  const [heatmapData, setHeatmapData] = useState<HeatmapData | null>(null);
  const [opacity, setOpacity] = useState(0.3);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationFrameRef = useRef<number>();

  // Initialize screen dimensions and start tracking
  const startTracking = useCallback(async () => {
    try {
      const screenWidth = window.screen.width;
      const screenHeight = window.screen.height;
      
      await invoke("start_heatmap_tracking", {
        screenWidth,
        screenHeight,
      });
      
      setIsTracking(true);
      console.log("[Heatmap] Started tracking with dimensions:", screenWidth, "x", screenHeight);
    } catch (error) {
      console.error("[Heatmap] Failed to start tracking:", error);
    }
  }, []);

  const stopTracking = useCallback(async () => {
    try {
      await invoke("stop_heatmap_tracking");
      setIsTracking(false);
      console.log("[Heatmap] Stopped tracking");
    } catch (error) {
      console.error("[Heatmap] Failed to stop tracking:", error);
    }
  }, []);

  const clearHeatmap = useCallback(async () => {
    try {
      await invoke("clear_heatmap_data");
      setHeatmapData(null);
      console.log("[Heatmap] Cleared data");
    } catch (error) {
      console.error("[Heatmap] Failed to clear data:", error);
    }
  }, []);

  // Fetch and update heatmap data
  const updateHeatmapData = useCallback(async () => {
    if (!isTracking) return;
    
    try {
      const data = await invoke<HeatmapData>("get_heatmap_data");
      setHeatmapData(data);
    } catch (error) {
      console.error("[Heatmap] Failed to get heatmap data:", error);
    }
  }, [isTracking]);

  // Render heatmap on canvas
  const renderHeatmap = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !heatmapData) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Set canvas size to screen dimensions
    canvas.width = heatmapData.screen_width;
    canvas.height = heatmapData.screen_height;

    // Clear canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Create gradient for heat visualization
    const createHeatGradient = (x: number, y: number, intensity: number) => {
      const radius = Math.min(50, intensity * 30); // Scale radius based on intensity
      const gradient = ctx.createRadialGradient(x, y, 0, x, y, radius);
      
      const alpha = Math.min(0.8, intensity * 0.4); // Scale opacity based on intensity
      gradient.addColorStop(0, `rgba(255, 0, 0, ${alpha})`); // Red center (hot)
      gradient.addColorStop(0.3, `rgba(255, 128, 0, ${alpha * 0.7})`); // Orange
      gradient.addColorStop(0.6, `rgba(255, 255, 0, ${alpha * 0.5})`); // Yellow
      gradient.addColorStop(1, `rgba(0, 255, 0, 0)`); // Transparent edge
      
      return gradient;
    };

    // Set blend mode for additive effect
    ctx.globalCompositeOperation = "screen";

    // Render points with intensity-based visualization
    const now = Date.now();
    heatmapData.points.forEach((point: HeatmapPoint) => {
      // Fade out older points
      const age = now - point.timestamp;
      const maxAge = 60000; // 60 seconds
      if (age > maxAge) return;
      
      const ageFactor = 1 - (age / maxAge);
      const effectiveIntensity = point.intensity * ageFactor;
      
      if (effectiveIntensity > 0.1) { // Only render points with significant intensity
        const gradient = createHeatGradient(point.x, point.y, effectiveIntensity);
        ctx.fillStyle = gradient;
        
        const radius = Math.min(50, effectiveIntensity * 30);
        ctx.beginPath();
        ctx.arc(point.x, point.y, radius, 0, 2 * Math.PI);
        ctx.fill();
      }
    });

    // Reset blend mode
    ctx.globalCompositeOperation = "source-over";
  }, [heatmapData]);

  // Listen for backend events
  useEffect(() => {
    const unlistenTracking = listen<[number, number]>(
      "heatmap-tracking-started",
      (event) => {
        const [screenWidth, screenHeight] = event.payload;
        setIsTracking(true);
        console.log("[Heatmap] Tracking started:", screenWidth, "x", screenHeight);
      }
    );

    const unlistenStopped = listen(
      "heatmap-tracking-stopped",
      () => {
        setIsTracking(false);
        console.log("[Heatmap] Tracking stopped");
      }
    );

    const unlistenCleared = listen(
      "heatmap-data-cleared",
      () => {
        setHeatmapData(null);
        console.log("[Heatmap] Data cleared");
      }
    );

    const unlistenPointAdded = listen<[number, number, number, string]>(
      "heatmap-point-added",
      () => {
        // Trigger data update when new points are added
        // We'll batch these updates to avoid too frequent refreshes
      }
    );

    return () => {
      Promise.all([
        unlistenTracking,
        unlistenStopped,
        unlistenCleared,
        unlistenPointAdded,
      ]).then((unlisteners) => {
        unlisteners.forEach((fn) => fn());
      });
    };
  }, []);

  // Update heatmap data periodically
  useEffect(() => {
    if (!isTracking) return;

    const interval = setInterval(updateHeatmapData, 1000); // Update every second
    return () => clearInterval(interval);
  }, [isTracking, updateHeatmapData]);

  // Render heatmap when data changes
  useEffect(() => {
    const render = () => {
      renderHeatmap();
      if (isTracking) {
        animationFrameRef.current = requestAnimationFrame(render);
      }
    };

    if (isTracking && heatmapData) {
      render();
    }

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [isTracking, heatmapData, renderHeatmap]);

  // Check tracking status on mount
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const tracking = await invoke<boolean>("is_heatmap_tracking");
        setIsTracking(tracking);
        if (tracking) {
          await updateHeatmapData();
        }
      } catch (error) {
        console.error("[Heatmap] Failed to check tracking status:", error);
      }
    };

    checkStatus();
  }, [updateHeatmapData]);

  return (
    <>
      {/* Control Panel */}
      <div
        className="heatmap-controls"
        style={{
          position: "fixed",
          top: "10px",
          right: "10px",
          zIndex: 1000000,
          backgroundColor: "rgba(0, 0, 0, 0.8)",
          color: "white",
          padding: "10px",
          borderRadius: "8px",
          fontSize: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "8px",
          minWidth: "200px",
        }}
      >
        <div style={{ fontWeight: "bold" }}>Cursor Heatmap</div>
        <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
          <button
            onClick={startTracking}
            disabled={isTracking}
            style={{
              padding: "4px 8px",
              backgroundColor: isTracking ? "#666" : "#22c55e",
              color: "white",
              border: "none",
              borderRadius: "4px",
              cursor: isTracking ? "not-allowed" : "pointer",
              fontSize: "11px",
            }}
          >
            Start
          </button>
          <button
            onClick={stopTracking}
            disabled={!isTracking}
            style={{
              padding: "4px 8px",
              backgroundColor: !isTracking ? "#666" : "#ef4444",
              color: "white",
              border: "none",
              borderRadius: "4px",
              cursor: !isTracking ? "not-allowed" : "pointer",
              fontSize: "11px",
            }}
          >
            Stop
          </button>
          <button
            onClick={clearHeatmap}
            style={{
              padding: "4px 8px",
              backgroundColor: "#f59e0b",
              color: "white",
              border: "none",
              borderRadius: "4px",
              cursor: "pointer",
              fontSize: "11px",
            }}
          >
            Clear
          </button>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <label style={{ fontSize: "11px" }}>Opacity:</label>
          <input
            type="range"
            min="0.1"
            max="1"
            step="0.1"
            value={opacity}
            onChange={(e) => setOpacity(parseFloat(e.target.value))}
            style={{ flex: 1 }}
          />
          <span style={{ fontSize: "11px", minWidth: "30px" }}>
            {Math.round(opacity * 100)}%
          </span>
        </div>
        <div style={{ fontSize: "11px", color: "#ccc" }}>
          Status: {isTracking ? "🟢 Tracking" : "🔴 Stopped"}
          {heatmapData && (
            <div>Points: {heatmapData.points.length}</div>
          )}
        </div>
      </div>

      {/* Heatmap Canvas Overlay */}
      {isTracking && (
        <canvas
          ref={canvasRef}
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            width: "100vw",
            height: "100vh",
            pointerEvents: "none",
            zIndex: 999998,
            opacity: opacity,
            mixBlendMode: "multiply",
          }}
        />
      )}
    </>
  );
};

export default HeatmapVisualizer;