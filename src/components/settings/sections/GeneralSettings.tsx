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
import { UI } from "@/lib/constants.generated";
import type { FloatingBarConfig } from "@/types/bar-config";

export default function GeneralSettings({ settings }: SettingsSectionProps) {
  const [autoLaunchEnabled, setAutoLaunchEnabled] = useState(false);
  const [autoLaunchLoading, setAutoLaunchLoading] = useState(false);
  const [restartOnboardingLoading, setRestartOnboardingLoading] =
    useState(false);
  const [onboardingInfo, setOnboardingInfo] = useState<any>(null);
  const [barAppearance, setBarAppearance] = useState<string>(
    UI.BAR_APPEARANCES_FLOATING
  );
  const [barAppearanceLoading, setBarAppearanceLoading] = useState(false);
  const [followCursorDisplay, setFollowCursorDisplay] = useState(true);
  const [followCursorLoading, setFollowCursorLoading] = useState(false);
  const [showGlowBorder, setShowGlowBorder] = useState(true);
  const [showGlowBorderLoading, setShowGlowBorderLoading] = useState(false);

  // Load auto-launch status and onboarding info on component mount
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        // Load auto-launch status
        const enabled = await invoke<boolean>("is_autostart_enabled");
        setAutoLaunchEnabled(enabled);

        // Load onboarding info
        const info = await invoke("get_onboarding_info");
        setOnboardingInfo(info);

        // Load current bar appearance and glow-border preference
        const barConfig = await invoke<{
          bar_appearance?: string;
          show_glow_border?: boolean;
        }>("ui_get_bar_config");
        if (barConfig?.bar_appearance) {
          setBarAppearance(barConfig.bar_appearance);
        }
        if (typeof barConfig?.show_glow_border === "boolean") {
          setShowGlowBorder(barConfig.show_glow_border);
        }

        // Load "follow cursor across displays"
        const barSettings = await invoke<{ follow_cursor_display?: boolean }>(
          "get_floating_bar_settings",
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
        await invoke<boolean>("enable_autostart");
        setAutoLaunchEnabled(true);
        console.log("Auto-launch enabled - Juno will start when you log in");
      } else {
        await invoke<boolean>("disable_autostart");
        setAutoLaunchEnabled(false);
        console.log("Auto-launch disabled");
      }
    } catch (error) {
      console.error("Failed to update auto-launch setting:", error);
      // Revert the state if the operation failed
      const currentStatus = await invoke<boolean>("is_autostart_enabled").catch(
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
      await invoke("restart_onboarding");
      toast.success("Onboarding restarted successfully", {
        description: "The onboarding window has been opened",
      });

      // Refresh onboarding info
      const info = await invoke("get_onboarding_info");
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
        "get_floating_bar_settings",
      );
      await invoke("set_floating_bar_settings", {
        settings: { ...current, follow_cursor_display: enabled },
      });
      setFollowCursorDisplay(enabled);
      toast.success(
        enabled
          ? "Bar will follow your cursor across displays"
          : "Bar will stay on its display",
      );
    } catch (error) {
      console.error("Failed to update follow-cursor setting:", error);
      toast.error("Failed to update setting", {
        description: error as string,
      });
    } finally {
      setFollowCursorLoading(false);
    }
  };

  const handleBarAppearanceChange = async (newAppearance: string) => {
    if (barAppearanceLoading) return;
    setBarAppearanceLoading(true);
    try {
      const currentConfig = await invoke<FloatingBarConfig>("ui_get_bar_config");
      const updatedConfig = {
        ...currentConfig,
        bar_appearance: newAppearance,
      };
      await invoke("ui_set_bar_config", { config: updatedConfig });
      setBarAppearance(newAppearance);
      toast.success("Bar appearance updated");
    } catch (error) {
      console.error("Failed to update bar appearance:", error);
      toast.error("Failed to update bar appearance", {
        description: error as string,
      });
    } finally {
      setBarAppearanceLoading(false);
    }
  };

  const handleShowGlowBorderChange = async (enabled: boolean) => {
    if (showGlowBorderLoading) return;
    setShowGlowBorderLoading(true);
    const previous = showGlowBorder;
    setShowGlowBorder(enabled);
    try {
      // Read-modify-write the whole bar config, like the appearance dropdown.
      const currentConfig = await invoke<FloatingBarConfig>("ui_get_bar_config");
      await invoke("ui_set_bar_config", {
        config: { ...currentConfig, show_glow_border: enabled },
      });
    } catch (error) {
      console.error("Failed to update glowing border setting:", error);
      setShowGlowBorder(previous);
      toast.error("Could not change that setting");
    } finally {
      setShowGlowBorderLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <SettingsGroup title="Startup">
        <SettingsRow
          htmlFor="auto-launch"
          label="Launch at login"
          description="Automatically start Juno when you log in to your computer"
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
          label="Sound effects"
          description="Play sounds for notifications and feedback"
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
        advanced
        footer="Bar windows switch styles immediately when changed."
      >
        <SettingsRow
          htmlFor="bar-appearance"
          label="Bar appearance"
          description="Which bar UI style to use in bar windows"
        >
          <Select
            value={barAppearance}
            onValueChange={handleBarAppearanceChange}
            disabled={barAppearanceLoading}
          >
            <SelectTrigger id="bar-appearance" className="w-[190px]">
              <SelectValue placeholder="Select bar appearance" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UI.BAR_APPEARANCES_FLOATING}>
                Floating (Standard)
              </SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_APP}>App Bar</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_VOICE_AI}>Voice AI</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_DYNAMIC}>Dynamic</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_ORB}>ElevenLabs Orb</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_REACT_ORB}>React Orb</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_PERSONA}>
                Persona (AI Avatar)
              </SelectItem>
            </SelectContent>
          </Select>
        </SettingsRow>
        <SettingsRow
          htmlFor="follow-cursor-display"
          label="Follow cursor across displays"
          description="Keep the bar on whichever display your cursor is on, so you never hunt for it"
        >
          <Switch
            id="follow-cursor-display"
            checked={followCursorDisplay}
            onCheckedChange={handleFollowCursorChange}
            disabled={followCursorLoading}
          />
        </SettingsRow>
        <SettingsRow
          htmlFor="show-glow-border"
          label="Show glowing border"
          description="Wrap the floating bar in a glowing border that lights up while Juno is listening or working"
        >
          <Switch
            id="show-glow-border"
            checked={showGlowBorder}
            onCheckedChange={handleShowGlowBorderChange}
            disabled={showGlowBorderLoading}
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
    invoke<BuildInfo>("get_build_info")
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
