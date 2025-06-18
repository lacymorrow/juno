import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  CheckCircle,
  MonitorSpeaker,
  RefreshCw,
  RotateCcw,
  Shield,
  Terminal,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { SettingsSectionProps } from "../types";

interface PermissionState {
  granted: boolean;
  required: boolean;
}

interface PermissionsState {
  accessibility: PermissionState;
  screenRecording: PermissionState;
  microphone: PermissionState;
  allGranted: boolean;
}

interface AdvancedSettingsProps extends SettingsSectionProps {
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}

export default function AdvancedSettings({
  settings,
  onNavigateToDevTools,
  onNavigateToChat,
  onNavigateToPermissions,
}: AdvancedSettingsProps) {
  const [permissionsState, setPermissionsState] =
    useState<PermissionsState | null>(null);
  const [permissionsLoading, setPermissionsLoading] = useState(false);
  const [debugMode, setDebugMode] = useState(false);

  // Visualization settings state
  const [showKeyPressOverlay, setShowKeyPressOverlay] = useState(
    localStorage.getItem("juno-show-key-press-overlay") === "true"
  );
  const [showCommandOverlay, setShowCommandOverlay] = useState(
    localStorage.getItem("juno-show-command-overlay") === "true"
  );
  const [showClickVisualization, setShowClickVisualization] = useState(
    localStorage.getItem("juno-show-click-visualization") !== "false" // Default to true
  );

  // Load debug mode status on mount
  useEffect(() => {
    const loadDebugMode = async () => {
      try {
        const enabled = await invoke("get_debug_mode");
        setDebugMode(enabled as boolean);
      } catch (error) {
        console.error("Failed to get debug mode status:", error);
      }
    };
    loadDebugMode();
  }, []);

  const loadPermissionsStatus = async () => {
    setPermissionsLoading(true);
    try {
      const result = await invoke<PermissionsState>("get_permissions_status");
      setPermissionsState(result);
    } catch (error) {
      console.error("Failed to load permissions:", error);
      setPermissionsState(null);
    } finally {
      setPermissionsLoading(false);
    }
  };

  const getPermissionIcon = (granted: boolean, required: boolean) => {
    if (!required) return <CheckCircle className="h-4 w-4 text-gray-400" />;
    return granted ? (
      <CheckCircle className="h-4 w-4 text-green-500" />
    ) : (
      <AlertCircle className="h-4 w-4 text-red-500" />
    );
  };

  const getPermissionBadge = (granted: boolean, required: boolean) => {
    if (!required) {
      return <Badge variant="secondary">Optional</Badge>;
    }
    return (
      <Badge variant={granted ? "default" : "destructive"}>
        {granted ? "Granted" : "Required"}
      </Badge>
    );
  };

  useEffect(() => {
    loadPermissionsStatus();
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Advanced</h3>

        {/* Developer Options */}
        <Card>
          <CardHeader>
            <CardTitle>Developer Options</CardTitle>
            <CardDescription>
              Advanced settings for developers and power users
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Debug Mode</div>
                <div className="text-sm text-gray-500">
                  Enable verbose logging and debug features
                </div>
              </div>
              <Switch
                checked={debugMode}
                onCheckedChange={async (enabled) => {
                  try {
                    await invoke("set_debug_mode", { enabled });
                    setDebugMode(enabled);
                    toast.success(
                      `Debug mode ${enabled ? "enabled" : "disabled"}`
                    );
                  } catch (error) {
                    toast.error("Failed to toggle debug mode");
                  }
                }}
              />
            </div>

            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Performance Monitoring</div>
                <div className="text-sm text-gray-500">
                  Monitor system resource usage
                </div>
              </div>
              <Switch
                checked={settings.performanceMonitoringEnabled}
                onCheckedChange={settings.handlePerformanceMonitoringChange}
              />
            </div>
          </CardContent>
        </Card>

        {/* Visualization Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <MonitorSpeaker size={20} />
              Visualization Settings
            </CardTitle>
            <CardDescription>
              Configure visual feedback for key presses and command execution
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Show Key Press Overlay</div>
                <div className="text-sm text-gray-500">
                  Display key presses in real-time during agent operation
                </div>
              </div>
              <Switch
                checked={showKeyPressOverlay}
                onCheckedChange={(enabled) => {
                  localStorage.setItem(
                    "juno-show-key-press-overlay",
                    enabled.toString()
                  );
                  setShowKeyPressOverlay(enabled);
                  toast.success(
                    `Key press overlay ${enabled ? "enabled" : "disabled"}`
                  );
                }}
              />
            </div>

            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Show Command Execution</div>
                <div className="text-sm text-gray-500">
                  Display active command status during tool execution
                </div>
              </div>
              <Switch
                checked={showCommandOverlay}
                onCheckedChange={(enabled) => {
                  localStorage.setItem(
                    "juno-show-command-overlay",
                    enabled.toString()
                  );
                  setShowCommandOverlay(enabled);
                  toast.success(
                    `Command overlay ${enabled ? "enabled" : "disabled"}`
                  );
                }}
              />
            </div>

            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Show Click Visualization</div>
                <div className="text-sm text-gray-500">
                  Display visual feedback for mouse clicks and interactions
                </div>
              </div>
              <Switch
                checked={showClickVisualization}
                onCheckedChange={(enabled) => {
                  localStorage.setItem(
                    "juno-show-click-visualization",
                    enabled.toString()
                  );
                  setShowClickVisualization(enabled);
                  toast.success(
                    `Click visualization ${enabled ? "enabled" : "disabled"}`
                  );
                }}
              />
            </div>

            <div className="text-sm text-muted-foreground">
              <p>
                <strong>Key Press Overlay:</strong> Shows keyboard input in the
                top-right corner
              </p>
              <p>
                <strong>Command Execution:</strong> Shows active tools and
                commands in the top-left corner
              </p>
              <p>
                <strong>Click Visualization:</strong> Shows animated circles
                where mouse clicks occur
              </p>
            </div>

            <div className="flex gap-2 pt-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  toast.info(
                    "Visualization features help you see what the AI agent is doing in real-time"
                  );
                }}
              >
                <AlertCircle className="h-4 w-4 mr-1" />
                Help
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* Developer Tools */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Terminal size={20} />
              Developer Tools
            </CardTitle>
            <CardDescription>
              Access developer tools and application windows
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <Button
                variant="outline"
                className="h-20 flex flex-col items-center justify-center gap-2"
                onClick={onNavigateToDevTools}
              >
                <Terminal size={24} />
                <div className="text-center">
                  <div className="font-medium">Developer Tools</div>
                  <div className="text-xs text-muted-foreground">
                    Debug and testing tools
                  </div>
                </div>
              </Button>

              <Button
                variant="outline"
                className="h-20 flex flex-col items-center justify-center gap-2"
                onClick={onNavigateToChat}
              >
                <MonitorSpeaker size={24} />
                <div className="text-center">
                  <div className="font-medium">Main Chat</div>
                  <div className="text-xs text-muted-foreground">
                    Return to main interface
                  </div>
                </div>
              </Button>
            </div>

            <p className="text-sm text-muted-foreground">
              You can also access Developer Tools from the system tray menu or
              use the toggle button in the main interface.
            </p>
          </CardContent>
        </Card>

        {/* Permissions Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield size={20} />
              macOS Permissions
            </CardTitle>
            <CardDescription>
              Manage system permissions required for AI computer use features
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {permissionsLoading ? (
              <div className="flex items-center gap-2 text-muted-foreground">
                <RefreshCw className="h-4 w-4 animate-spin" />
                <span>Checking permissions...</span>
              </div>
            ) : permissionsState ? (
              <div className="space-y-3">
                {/* Overall Status */}
                <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
                  <div className="flex items-center gap-2">
                    {permissionsState.allGranted ? (
                      <CheckCircle className="h-5 w-5 text-green-500" />
                    ) : (
                      <AlertCircle className="h-5 w-5 text-red-500" />
                    )}
                    <span className="font-medium">
                      {permissionsState.allGranted
                        ? "All permissions granted"
                        : "Some permissions missing"}
                    </span>
                  </div>
                  <Badge
                    variant={
                      permissionsState.allGranted ? "default" : "destructive"
                    }
                  >
                    {permissionsState.allGranted ? "Ready" : "Needs Setup"}
                  </Badge>
                </div>

                {/* Individual Permissions */}
                <div className="grid gap-2">
                  {/* Accessibility */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.accessibility.granted,
                        permissionsState.accessibility.required
                      )}
                      <span className="text-sm">Accessibility</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.accessibility.granted,
                      permissionsState.accessibility.required
                    )}
                  </div>

                  {/* Screen Recording */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.screenRecording.granted,
                        permissionsState.screenRecording.required
                      )}
                      <span className="text-sm">Screen Recording</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.screenRecording.granted,
                      permissionsState.screenRecording.required
                    )}
                  </div>

                  {/* Microphone */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.microphone.granted,
                        permissionsState.microphone.required
                      )}
                      <span className="text-sm">Microphone</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.microphone.granted,
                      permissionsState.microphone.required
                    )}
                  </div>
                </div>

                {/* Action Buttons */}
                <div className="flex gap-2 pt-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={loadPermissionsStatus}
                    disabled={permissionsLoading}
                  >
                    <RefreshCw className="h-4 w-4 mr-1" />
                    Refresh
                  </Button>
                  <Button
                    onClick={onNavigateToPermissions}
                    size="sm"
                    variant={
                      permissionsState.allGranted ? "outline" : "default"
                    }
                  >
                    <Shield className="h-4 w-4 mr-1" />
                    {permissionsState.allGranted
                      ? "View Details"
                      : "Setup Permissions"}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="text-center py-4 text-muted-foreground">
                <p>Unable to check permissions status</p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={loadPermissionsStatus}
                  className="mt-2"
                >
                  <RefreshCw className="h-4 w-4 mr-1" />
                  Try Again
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Reset Settings */}
        <Card>
          <CardHeader>
            <CardTitle>Reset Settings</CardTitle>
            <CardDescription>
              Reset all settings to their default values
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button
              variant="destructive"
              onClick={async () => {
                if (
                  confirm(
                    "Are you sure you want to reset all settings? This action cannot be undone."
                  )
                ) {
                  try {
                    await invoke("reset_all_settings");
                    await settings.loadAllSettings();
                    toast.success("All settings have been reset to defaults");
                  } catch (error) {
                    toast.error("Failed to reset settings");
                  }
                }
              }}
              className="w-full"
            >
              <RotateCcw className="w-4 h-4 mr-2" />
              Reset All Settings
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
