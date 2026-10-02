import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { Check, Copy, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { COMMANDS } from "@/lib/constants.generated";
import { AppearancePicker } from "../AppearancePicker";
import { useBarAppearance } from "../useBarAppearance";
import { UpdatesGroup } from "../UpdatesGroup";

export default function GeneralSettings({ settings }: SettingsSectionProps) {
  const [autoLaunchEnabled, setAutoLaunchEnabled] = useState(false);
  const [autoLaunchLoading, setAutoLaunchLoading] = useState(false);
  const [restartOnboardingLoading, setRestartOnboardingLoading] =
    useState(false);
  const [onboardingInfo, setOnboardingInfo] = useState<any>(null);
  const {
    value: barAppearance,
    saving: barAppearanceLoading,
    change: handleBarAppearanceChange,
  } = useBarAppearance();
  const [followCursorDisplay, setFollowCursorDisplay] = useState(true);
  const [followCursorLoading, setFollowCursorLoading] = useState(false);

  // Load auto-launch status and onboarding info on component mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        // Load auto-launch status
        const enabled = await invoke<boolean>(COMMANDS.AUTOSTART_IS_AUTOSTART_ENABLED);
        setAutoLaunchEnabled(enabled);

        // Load onboarding info
        const info = await invoke(COMMANDS.ONBOARDING_GET_ONBOARDING_INFO);
        setOnboardingInfo(info);

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

  const handleRestartOnboarding = async () => {
    if (restartOnboardingLoading) return;

    setRestartOnboardingLoading(true);

    try {
      await invoke(COMMANDS.ONBOARDING_RESTART_ONBOARDING);
      toast.success("Onboarding restarted successfully", {
        description: "The onboarding window has been opened",
      });

      // Refresh onboarding info
      const info = await invoke(COMMANDS.ONBOARDING_GET_ONBOARDING_INFO);
      setOnboardingInfo(info);
    } catch (error) {
      console.error("Failed to restart onboarding:", error);
      toast.error("Failed to restart onboarding", {
        description: error as string,
      });
    } finally {
      setRestartOnboardingLoading(false);
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

      <SettingsGroup title="Sound">
        <SettingsRow
          htmlFor="sound-enabled"
          label="Play sounds"
          description="A soft cue when dictation starts and stops."
        >
          <Switch
            id="sound-enabled"
            checked={settings.soundEnabled}
            onCheckedChange={settings.handleSoundEnabledChange}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Agent"
        advanced
        footer="Multi-agent mode uses specialized agents for different tasks; single-agent mode uses one agent for everything."
      >
        <SettingsRow
          htmlFor="agent-mode"
          label="Agent mode"
          description="How Juno divides up tasks"
        >
          <Select
            value={settings.agentMode}
            onValueChange={settings.handleAgentModeChange}
          >
            <SelectTrigger id="agent-mode" className="w-[190px]">
              <SelectValue placeholder="Select agent mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="multi">Multi-Agent (Recommended)</SelectItem>
              <SelectItem value="single">Single Agent</SelectItem>
            </SelectContent>
          </Select>
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

      <SettingsGroup
        title="Onboarding"
        advanced
        footer={
          onboardingInfo?.completed_at
            ? `Last completed ${new Date(onboardingInfo.completed_at).toLocaleDateString()}.`
            : undefined
        }
      >
        <SettingsRow
          id="restart-onboarding"
          label="Restart onboarding"
          description={
            onboardingInfo?.is_development_mode
              ? "Go through the welcome guide again. Development mode: onboarding always shows on restart."
              : "Go through the welcome guide and setup process again"
          }
        >
          <Button
            onClick={handleRestartOnboarding}
            disabled={restartOnboardingLoading}
            variant="outline"
            size="sm"
          >
            {restartOnboardingLoading ? (
              <>
                <RotateCcw className="mr-2 h-4 w-4 animate-spin" />
                Restarting…
              </>
            ) : (
              <>
                <RotateCcw className="mr-2 h-4 w-4" />
                Restart
              </>
            )}
          </Button>
        </SettingsRow>
      </SettingsGroup>

      <UpdatesGroup />

      <BuildGroup />
    </div>
  );
}

interface BuildInfo {
  version: string;
  build: string;
  commit: string;
  branch: string;
  built_at: string;
  dirty: boolean;
  demo: boolean;
  cohort: string | null;
}

/**
 * Which build this is, in one copyable line.
 *
 * Every DMG used to be "Juno 0.7.0", so a bug report could only name a date
 * and two builds from the same day were indistinguishable. This says the
 * commit, and one click puts it on the clipboard for the report.
 */
function BuildGroup() {
  const [info, setInfo] = useState<BuildInfo | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    invoke<BuildInfo>(COMMANDS.APP_GET_BUILD_INFO)
      .then(setInfo)
      .catch((error) => console.error("Failed to read build info:", error));
  }, []);

  if (!info) return null;

  const label =
    `${info.version} (${info.build}) ${info.commit}` +
    (info.dirty ? " dirty" : "") +
    (info.demo ? ` · demo${info.cohort ? ` ${info.cohort}` : ""}` : "");

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(label);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      toast.error("Could not copy the build ID");
    }
  };

  return (
    <SettingsGroup title="Build">
      <SettingsRow
        label={label}
        description={`${info.branch}, built ${new Date(info.built_at).toLocaleString()}`}
      >
        <Button variant="outline" size="sm" onClick={copy}>
          {copied ? (
            <>
              <Check className="mr-2 h-4 w-4" />
              Copied
            </>
          ) : (
            <>
              <Copy className="mr-2 h-4 w-4" />
              Copy
            </>
          )}
        </Button>
      </SettingsRow>
    </SettingsGroup>
  );
}
