import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { toast } from "sonner";
import { COMMANDS } from "@/lib/constants.generated";
import { AppearancePicker } from "../AppearancePicker";
import { useBarAppearance } from "../useBarAppearance";
import { UpdatesGroup } from "../UpdatesGroup";

export default function GeneralSettings(_props: SettingsSectionProps) {
  const [autoLaunchEnabled, setAutoLaunchEnabled] = useState(false);
  const [autoLaunchLoading, setAutoLaunchLoading] = useState(false);
  const {
    value: barAppearance,
    saving: barAppearanceLoading,
    change: handleBarAppearanceChange,
  } = useBarAppearance();
  const [followCursorDisplay, setFollowCursorDisplay] = useState(true);
  const [followCursorLoading, setFollowCursorLoading] = useState(false);

  // Load auto-launch status on component mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        // Load auto-launch status
        const enabled = await invoke<boolean>(COMMANDS.AUTOSTART_IS_AUTOSTART_ENABLED);
        setAutoLaunchEnabled(enabled);

        // Load "follow cursor across displays"
        const barSettings = await invoke<{ follow_cursor_display?: boolean }>(
          COMMANDS.SETTINGS_GET_FLOATING_BAR_SETTINGS,
        );
        if (typeof barSettings?.follow_cursor_display === "boolean") {
          setFollowCursorDisplay(barSettings.follow_cursor_display);
        }
      } catch (error) {
        console.error("Failed to load initial data:", error);
        // Default to false if unable to determine status
        setAutoLaunchEnabled(false);
      }
    };

    loadInitialData();
  }, []);

  const handleAutoLaunchChange = async (enabled: boolean) => {
    if (autoLaunchLoading) return;

    setAutoLaunchLoading(true);

    try {
      if (enabled) {
        await invoke<boolean>(COMMANDS.AUTOSTART_ENABLE_AUTOSTART);
        setAutoLaunchEnabled(true);
        console.log("Auto-launch enabled - Juno will start when you log in");
      } else {
        await invoke<boolean>(COMMANDS.AUTOSTART_DISABLE_AUTOSTART);
        setAutoLaunchEnabled(false);
        console.log("Auto-launch disabled");
      }
    } catch (error) {
      console.error("Failed to update auto-launch setting:", error);
      // Revert the state if the operation failed
      const currentStatus = await invoke<boolean>(COMMANDS.AUTOSTART_IS_AUTOSTART_ENABLED).catch(
        () => false
      );
      setAutoLaunchEnabled(currentStatus);
    } finally {
      setAutoLaunchLoading(false);
    }
  };

  const handleFollowCursorChange = async (enabled: boolean) => {
    if (followCursorLoading) return;
    setFollowCursorLoading(true);
    try {
      // Read-modify-write: this command takes the whole settings object.
      const current = await invoke<Record<string, unknown>>(
        COMMANDS.SETTINGS_GET_FLOATING_BAR_SETTINGS,
      );
      await invoke(COMMANDS.SETTINGS_SET_FLOATING_BAR_SETTINGS, {
        settings: { ...current, follow_cursor_display: enabled },
      });
      setFollowCursorDisplay(enabled);
    } catch (error) {
      console.error("Failed to update follow-cursor setting:", error);
      toast.error("Failed to update setting", {
        description: error as string,
      });
    } finally {
      setFollowCursorLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <SettingsGroup title="Startup">
        <SettingsRow
          htmlFor="auto-launch"
          label="Open at login"
          description="Juno is in the menu bar as soon as you log in."
        >
          <Switch
            id="auto-launch"
            checked={autoLaunchEnabled}
            onCheckedChange={handleAutoLaunchChange}
            disabled={autoLaunchLoading}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Appearance"
        footer="Your bar changes as you browse. Every look goes through the same moments: resting, listening, dictating, done."
      >
        <SettingsRow
          id="bar-appearance"
          below={
            <AppearancePicker
              value={barAppearance}
              onChange={handleBarAppearanceChange}
              disabled={barAppearanceLoading}
            />
          }
        />
        <SettingsRow
          htmlFor="follow-cursor-display"
          label="Follow me across displays"
          description="The bar moves to whichever display your cursor is on."
        >
          <Switch
            id="follow-cursor-display"
            checked={followCursorDisplay}
            onCheckedChange={handleFollowCursorChange}
            disabled={followCursorLoading}
          />
        </SettingsRow>
      </SettingsGroup>

      <UpdatesGroup />

    </div>
  );
}
