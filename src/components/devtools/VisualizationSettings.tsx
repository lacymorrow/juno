import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  MonitorSpeaker,
  Eye,
  AlertCircle,
  TestTube,
  MousePointer,
} from "lucide-react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";

const VisualizationSettings = () => {
  const [showKeyPressOverlay, setShowKeyPressOverlay] = useState(
    localStorage.getItem("juno-show-key-press-overlay") === "true"
  );
  const [showCommandOverlay, setShowCommandOverlay] = useState(true);
  const [showClickVisualization, setShowClickVisualization] = useState(
    localStorage.getItem("juno-show-click-visualization") !== "false" // Default to true
  );
  const [showDesktopCursorVisualization, setShowDesktopCursorVisualization] =
    useState(
      localStorage.getItem("juno-show-desktop-cursor-visualization") !== "false" // Default to true
    );

  // Load command overlay setting from Tauri settings
  useEffect(() => {
    const loadCommandOverlaySetting = async () => {
      try {
        const enabled = await invoke<boolean>("get_command_overlay_enabled");
        setShowCommandOverlay(enabled);
      } catch (error) {
        console.warn("Failed to load command overlay setting:", error);
        setShowCommandOverlay(true); // Default to enabled
      }
    };

    loadCommandOverlaySetting();
  }, []);

  const visualizationSettings = [
    {
      key: "keypress",
      title: "Key Press Overlay",
      description: "Display key presses in real-time during agent operation",
      storageKey: "juno-show-key-press-overlay",
      value: showKeyPressOverlay,
      setValue: setShowKeyPressOverlay,
      location: "Top-right corner",
      icon: <Eye className="h-4 w-4" />,
      useLocalStorage: true,
    },
    {
      key: "command",
      title: "Command Execution",
      description: "Display active command status during tool execution",
      storageKey: "", // Not used for Tauri settings
      value: showCommandOverlay,
      setValue: setShowCommandOverlay,
      location: "Top-left corner",
      icon: <MonitorSpeaker className="h-4 w-4" />,
      useLocalStorage: false, // Uses Tauri settings
    },
    {
      key: "click",
      title: "Click Visualization",
      description: "Display visual feedback for mouse clicks and interactions",
      storageKey: "juno-show-click-visualization",
      value: showClickVisualization,
      setValue: setShowClickVisualization,
      location: "At click locations",
      icon: <TestTube className="h-4 w-4" />,
      useLocalStorage: true,
    },
    {
      key: "desktop-cursor",
      title: "Desktop Cursor Overlay",
      description:
        "Show desktop-level cursor visualization with circles and ripples",
      storageKey: "juno-show-desktop-cursor-visualization",
      value: showDesktopCursorVisualization,
      setValue: setShowDesktopCursorVisualization,
      location: "Desktop-wide overlay",
      icon: <MousePointer className="h-4 w-4" />,
      useLocalStorage: true,
    },
  ];

  const handleToggleSetting = async (
    setting: (typeof visualizationSettings)[0],
    enabled: boolean
  ) => {
    if (setting.useLocalStorage) {
      // Use localStorage for settings that don't have backend commands yet
      localStorage.setItem(setting.storageKey, enabled.toString());
    } else {
      // Use Tauri settings for command overlay
      try {
        await invoke("set_command_overlay_enabled", { enabled });
      } catch (error) {
        console.error("Failed to save command overlay setting:", error);
        toast.error("Failed to save setting");
        return;
      }
    }

    setting.setValue(enabled);
    toast.success(`${setting.title} ${enabled ? "enabled" : "disabled"}`);
  };

  const handleEnableAll = () => {
    visualizationSettings.forEach((setting) => {
      handleToggleSetting(setting, true);
    });
    toast.success("All visualization features enabled");
  };

  const handleDisableAll = () => {
    visualizationSettings.forEach((setting) => {
      handleToggleSetting(setting, false);
    });
    toast.success("All visualization features disabled");
  };

  const enabledCount = visualizationSettings.filter((s) => s.value).length;

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <MonitorSpeaker size={20} />
            Visualization Settings
            <Badge variant="outline">
              {enabledCount}/{visualizationSettings.length}
            </Badge>
          </CardTitle>
          <CardDescription>
            Configure visual feedback for agent operations and testing
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {visualizationSettings.map((setting) => (
            <div
              key={setting.key}
              className="flex items-center justify-between p-3 border rounded-lg"
            >
              <div className="flex items-start gap-3">
                <div className="text-blue-600 mt-1">{setting.icon}</div>
                <div>
                  <div className="font-medium flex items-center gap-2">
                    {setting.title}
                    {setting.value && (
                      <Badge variant="secondary" className="text-xs">
                        Active
                      </Badge>
                    )}
                  </div>
                  <div className="text-sm text-gray-500">
                    {setting.description}
                  </div>
                  <div className="text-xs text-gray-400 mt-1">
                    Location: {setting.location}
                  </div>
                </div>
              </div>
              <Switch
                checked={setting.value}
                onCheckedChange={(enabled) =>
                  handleToggleSetting(setting, enabled)
                }
              />
            </div>
          ))}

          <div className="flex gap-2 pt-4 border-t">
            <Button
              variant="outline"
              size="sm"
              onClick={handleEnableAll}
              disabled={enabledCount === visualizationSettings.length}
            >
              Enable All
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={handleDisableAll}
              disabled={enabledCount === 0}
            >
              Disable All
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                toast.info(
                  "Visualization features help you see what the AI agent is doing in real-time during development and testing"
                );
              }}
            >
              <AlertCircle className="h-4 w-4 mr-1" />
              Help
            </Button>
          </div>

          <div className="text-sm text-muted-foreground space-y-1 pt-2 border-t">
            <p>
              <strong>Key Press Overlay:</strong> Shows keyboard input in the
              top-right corner
            </p>
            <p>
              <strong>Command Execution:</strong> Shows active tools and
              commands in the top-left corner
            </p>
            <p>
              <strong>Click Visualization:</strong> Shows animated circles where
              mouse clicks occur
            </p>
            <p>
              <strong>Desktop Cursor Overlay:</strong> Shows desktop-level
              cursor visualization with pulsing circles and ripple effects
            </p>
            <p className="text-xs text-amber-600 mt-2">
              <AlertCircle className="h-3 w-3 inline mr-1" />
              These settings are for development and testing purposes only
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};

export default VisualizationSettings;
