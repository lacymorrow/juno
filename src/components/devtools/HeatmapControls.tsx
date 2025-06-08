import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Activity, Play, Square, Trash2, Eye, EyeOff } from "lucide-react";

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

const HeatmapControls = () => {
  const [isTracking, setIsTracking] = useState(false);
  const [heatmapData, setHeatmapData] = useState<HeatmapData | null>(null);
  const [screenWidth, setScreenWidth] = useState(1920);
  const [screenHeight, setScreenHeight] = useState(1080);
  const [isVisible, setIsVisible] = useState(true);

  // Check initial status
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const tracking = await invoke<boolean>("is_heatmap_tracking");
        setIsTracking(tracking);
        
        if (tracking) {
          const data = await invoke<HeatmapData>("get_heatmap_data");
          setHeatmapData(data);
        }
        
        // Set screen dimensions from actual screen
        setScreenWidth(window.screen.width);
        setScreenHeight(window.screen.height);
      } catch (error) {
        console.error("[HeatmapControls] Failed to check status:", error);
      }
    };

    checkStatus();
  }, []);

  // Listen for heatmap events
  useEffect(() => {
    const unlistenTracking = listen<[number, number]>(
      "heatmap-tracking-started",
      (event) => {
        const [width, height] = event.payload;
        setIsTracking(true);
        setScreenWidth(width);
        setScreenHeight(height);
        console.log("[HeatmapControls] Tracking started:", width, "x", height);
      }
    );

    const unlistenStopped = listen(
      "heatmap-tracking-stopped",
      () => {
        setIsTracking(false);
        console.log("[HeatmapControls] Tracking stopped");
      }
    );

    const unlistenCleared = listen(
      "heatmap-data-cleared",
      () => {
        setHeatmapData(null);
        console.log("[HeatmapControls] Data cleared");
      }
    );

    const unlistenPointAdded = listen<[number, number, number, string]>(
      "heatmap-point-added",
      async () => {
        // Update data when points are added
        if (isTracking) {
          try {
            const data = await invoke<HeatmapData>("get_heatmap_data");
            setHeatmapData(data);
          } catch (error) {
            console.error("[HeatmapControls] Failed to update data:", error);
          }
        }
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
  }, [isTracking]);

  const handleStartTracking = async () => {
    try {
      await invoke("start_heatmap_tracking", {
        screenWidth,
        screenHeight,
      });
    } catch (error) {
      console.error("[HeatmapControls] Failed to start tracking:", error);
    }
  };

  const handleStopTracking = async () => {
    try {
      await invoke("stop_heatmap_tracking");
    } catch (error) {
      console.error("[HeatmapControls] Failed to stop tracking:", error);
    }
  };

  const handleClearData = async () => {
    try {
      await invoke("clear_heatmap_data");
    } catch (error) {
      console.error("[HeatmapControls] Failed to clear data:", error);
    }
  };

  const handleRefreshData = async () => {
    try {
      const data = await invoke<HeatmapData>("get_heatmap_data");
      setHeatmapData(data);
    } catch (error) {
      console.error("[HeatmapControls] Failed to refresh data:", error);
    }
  };

  const getStatusBadge = () => {
    if (isTracking) {
      return <Badge variant="default" className="bg-green-500">🟢 Tracking</Badge>;
    } else {
      return <Badge variant="secondary">🔴 Stopped</Badge>;
    }
  };

  const formatDuration = (startTime: number) => {
    const duration = Date.now() - startTime;
    const seconds = Math.floor(duration / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);

    if (hours > 0) {
      return `${hours}h ${minutes % 60}m ${seconds % 60}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${seconds % 60}s`;
    } else {
      return `${seconds}s`;
    }
  };

  const getEventTypeStats = () => {
    if (!heatmapData) return {};
    
    const stats: Record<string, number> = {};
    heatmapData.points.forEach(point => {
      stats[point.event_type] = (stats[point.event_type] || 0) + 1;
    });
    
    return stats;
  };

  const eventTypeStats = getEventTypeStats();

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Activity className="h-5 w-5" />
          Cursor Heatmap Tracker
        </CardTitle>
        <CardDescription>
          Track and visualize cursor movement and clicks during agent execution
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Status */}
        <div className="flex items-center justify-between">
          <Label>Status</Label>
          {getStatusBadge()}
        </div>

        {/* Screen Dimensions */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <Label htmlFor="screen-width">Screen Width</Label>
            <Input
              id="screen-width"
              type="number"
              value={screenWidth}
              onChange={(e) => setScreenWidth(parseInt(e.target.value) || 1920)}
              disabled={isTracking}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="screen-height">Screen Height</Label>
            <Input
              id="screen-height"
              type="number"
              value={screenHeight}
              onChange={(e) => setScreenHeight(parseInt(e.target.value) || 1080)}
              disabled={isTracking}
            />
          </div>
        </div>

        {/* Controls */}
        <div className="flex gap-2 flex-wrap">
          <Button
            onClick={handleStartTracking}
            disabled={isTracking}
            size="sm"
            className="flex items-center gap-2"
          >
            <Play className="h-4 w-4" />
            Start Tracking
          </Button>
          <Button
            onClick={handleStopTracking}
            disabled={!isTracking}
            variant="destructive"
            size="sm"
            className="flex items-center gap-2"
          >
            <Square className="h-4 w-4" />
            Stop Tracking
          </Button>
          <Button
            onClick={handleClearData}
            variant="outline"
            size="sm"
            className="flex items-center gap-2"
          >
            <Trash2 className="h-4 w-4" />
            Clear Data
          </Button>
          <Button
            onClick={handleRefreshData}
            variant="outline"
            size="sm"
            disabled={!isTracking}
          >
            Refresh
          </Button>
        </div>

        <Separator />

        {/* Statistics */}
        {heatmapData && (
          <div className="space-y-3">
            <Label className="text-sm font-medium">Statistics</Label>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-muted-foreground">Total Points:</span>
                <span className="ml-2 font-medium">{heatmapData.points.length}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Grid Size:</span>
                <span className="ml-2 font-medium">{heatmapData.grid_size}px</span>
              </div>
              <div>
                <span className="text-muted-foreground">Screen:</span>
                <span className="ml-2 font-medium">
                  {heatmapData.screen_width}x{heatmapData.screen_height}
                </span>
              </div>
              <div>
                <span className="text-muted-foreground">Duration:</span>
                <span className="ml-2 font-medium">
                  {formatDuration(heatmapData.start_time)}
                </span>
              </div>
            </div>

            {/* Event Type Breakdown */}
            {Object.keys(eventTypeStats).length > 0 && (
              <div className="space-y-2">
                <Label className="text-sm font-medium">Event Types</Label>
                <div className="space-y-1">
                  {Object.entries(eventTypeStats).map(([type, count]) => (
                    <div key={type} className="flex justify-between text-sm">
                      <span className="capitalize text-muted-foreground">{type}:</span>
                      <span className="font-medium">{count}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Last Update */}
            <div className="text-xs text-muted-foreground">
              Last updated: {new Date(heatmapData.last_update).toLocaleTimeString()}
            </div>
          </div>
        )}

        {!heatmapData && !isTracking && (
          <div className="text-center py-4 text-muted-foreground">
            Start tracking to see heatmap data
          </div>
        )}

        <Separator />

        {/* Usage Instructions */}
        <div className="space-y-2">
          <Label className="text-sm font-medium">How to Use</Label>
          <ul className="text-xs text-muted-foreground space-y-1">
            <li>• Click "Start Tracking" to begin recording cursor activity</li>
            <li>• Move your mouse and click around to generate heat data</li>
            <li>• The heatmap visualizer will show as an overlay when active</li>
            <li>• Use during agent execution to see where clicks occur</li>
            <li>• Different click types have different heat intensities</li>
          </ul>
        </div>
      </CardContent>
    </Card>
  );
};

export default HeatmapControls;