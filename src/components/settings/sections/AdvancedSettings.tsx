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
import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { SettingsSectionProps } from "../types";

interface AdvancedSettingsProps extends SettingsSectionProps {
  onNavigateToPermissions?: () => void;
}

export default function AdvancedSettings({
  settings,
  onNavigateToPermissions,
}: AdvancedSettingsProps) {
  const [debugMode, setDebugMode] = useState(false);

  // Suppress unused parameter warning for onNavigateToPermissions
  void onNavigateToPermissions;

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

  return (
    <div className="space-y-6">
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
  );
}
