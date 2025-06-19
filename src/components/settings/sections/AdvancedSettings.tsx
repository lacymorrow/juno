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
  onNavigateToPermissions?: () => void;
}

export default function AdvancedSettings({
  settings,
  onNavigateToPermissions,
}: AdvancedSettingsProps) {
  const [permissionsState, setPermissionsState] =
    useState<PermissionsState | null>(null);
  const [permissionsLoading, setPermissionsLoading] = useState(false);
  const [debugMode, setDebugMode] = useState(false);

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
