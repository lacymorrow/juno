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
import { COMMANDS } from "@/lib/constants.generated";

export default function AdvancedSettings({
  settings,
}: SettingsSectionProps) {
  const [debugMode, setDebugMode] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);

  // Load debug mode status on mount
  useEffect(() => {
    const loadDebugMode = async () => {
      try {
        const enabled = await invoke(COMMANDS.CORE_GET_DEBUG_MODE);
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
                  await invoke(COMMANDS.CORE_SET_DEBUG_MODE, { enabled });
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
        <CardContent className="space-y-3">
          {confirmReset ? (
            <div className="space-y-3">
              <p className="text-sm text-destructive">
                This will reset all settings to their defaults. This cannot be undone.
              </p>
              <div className="flex gap-2">
                <Button
                  variant="destructive"
                  onClick={async () => {
                    try {
                      await invoke(COMMANDS.SETTINGS_RESET_SETTINGS);
                      await settings.loadAllSettings();
                      toast.success("All settings have been reset to defaults");
                    } catch (error) {
                      toast.error("Failed to reset settings");
                    } finally {
                      setConfirmReset(false);
                    }
                  }}
                  className="flex-1"
                >
                  <RotateCcw className="w-4 h-4 mr-2" />
                  Confirm Reset
                </Button>
                <Button
                  variant="outline"
                  onClick={() => setConfirmReset(false)}
                  className="flex-1"
                >
                  Cancel
                </Button>
              </div>
            </div>
          ) : (
            <Button
              variant="destructive"
              onClick={() => setConfirmReset(true)}
              className="w-full"
            >
              <RotateCcw className="w-4 h-4 mr-2" />
              Reset All Settings
            </Button>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
