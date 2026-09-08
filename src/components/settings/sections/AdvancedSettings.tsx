import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";
import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { COMMANDS } from "@/lib/constants.generated";

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
      <SettingsGroup
        title="Developer Options"
        footer="Advanced settings for developers and power users."
      >
        <SettingsRow
          htmlFor="debug-mode"
          label="Debug Mode"
          description="Enable verbose logging and debug features"
        >
          <Switch
            id="debug-mode"
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
        </SettingsRow>

        <SettingsRow
          htmlFor="performance-monitoring"
          label="Performance Monitoring"
          description="Monitor system resource usage"
        >
          <Switch
            id="performance-monitoring"
            checked={settings.performanceMonitoringEnabled}
            onCheckedChange={settings.handlePerformanceMonitoringChange}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Reset Settings">
        <SettingsRow
          label="Reset all settings"
          description="Reset all settings to their default values"
          below={
            <Button
              variant="destructive"
              onClick={async () => {
                if (
                  confirm(
                    "Are you sure you want to reset all settings? This action cannot be undone."
                  )
                ) {
                  try {
                    await invoke(COMMANDS.SETTINGS_RESET_SETTINGS);
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
          }
        />
      </SettingsGroup>
    </div>
  );
}
