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
import { SettingsGroup, SettingsRow } from "../SettingsPrimitives";
import { Slider } from "@/components/ui/slider";
import { RotateCcw } from "lucide-react";
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
  const [bigCursorEnabled, setBigCursorEnabled] = useState(true);
  const [bigCursorScale, setBigCursorScale] = useState(3.0);
  const [bigCursorLoading, setBigCursorLoading] = useState(false);
  const [companionMode, setCompanionMode] = useState(false);
  const [companionModeLoading, setCompanionModeLoading] = useState(false);
  const [systemCursorSize, setSystemCursorSize] = useState(1.0);

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

        // Load current bar appearance
        const barConfig = await invoke<{
          bar_appearance?: string;
        }>("ui_get_bar_config");
        if (barConfig?.bar_appearance) {
          setBarAppearance(barConfig.bar_appearance);
        }

        // Load big cursor settings
        const cursorEnabled = await invoke<boolean>("get_big_cursor_enabled");
        setBigCursorEnabled(cursorEnabled);
        const cursorScale = await invoke<number>("get_big_cursor_scale");
        setBigCursorScale(cursorScale);

        // Load companion mode
        const companionEnabled = await invoke<boolean>("get_companion_mode");
        setCompanionMode(companionEnabled);
        const sysSize = await invoke<number>("get_system_cursor_size");
        setSystemCursorSize(sysSize);
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

  const handleBigCursorEnabledChange = async (enabled: boolean) => {
    if (bigCursorLoading) return;
    setBigCursorLoading(true);
    try {
      await invoke("set_big_cursor_enabled", { enabled });
      setBigCursorEnabled(enabled);
      if (!enabled) {
        const sysSize = await invoke<number>("get_system_cursor_size");
        setSystemCursorSize(sysSize);
      }
    } catch (error) {
      console.error("Failed to update big cursor setting:", error);
      toast.error("Failed to update big cursor setting");
    } finally {
      setBigCursorLoading(false);
    }
  };

  const handleBigCursorScaleChange = (value: number[]) => {
    setBigCursorScale(value[0]);
  };

  const handleBigCursorScaleCommit = async (value: number[]) => {
    const scale = value[0];
    try {
      await invoke("set_big_cursor_scale", { scale });
    } catch (error) {
      console.error("Failed to persist big cursor scale:", error);
      toast.error("Failed to update cursor scale");
    }
  };

  const handleCompanionModeChange = async (enabled: boolean) => {
    if (companionModeLoading) return;
    setCompanionModeLoading(true);
    try {
      await invoke("set_companion_mode", { enabled });
      setCompanionMode(enabled);
    } catch (error) {
      console.error("Failed to update companion mode:", error);
      toast.error("Failed to update companion mode");
    } finally {
      setCompanionModeLoading(false);
    }
  };

  return (
    <>
      <SettingsGroup title="Startup">
        <SettingsRow
          htmlFor="auto-launch"
          label="Launch at login"
          description="Automatically start Juno when you log in to your computer"
          control={
            <Switch
              id="auto-launch"
              checked={autoLaunchEnabled}
              onCheckedChange={handleAutoLaunchChange}
              disabled={autoLaunchLoading}
            />
          }
        />
      </SettingsGroup>

      <SettingsGroup title="Appearance">
        <SettingsRow
          htmlFor="bar-appearance"
          label="Bar style"
          description="Bar windows switch styles immediately when changed"
          control={
            <Select
              value={barAppearance}
              onValueChange={handleBarAppearanceChange}
              disabled={barAppearanceLoading}
            >
              <SelectTrigger id="bar-appearance" className="w-44">
                <SelectValue placeholder="Select bar appearance" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UI.BAR_APPEARANCES_FLOATING}>
                  Floating (Standard)
                </SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_APP}>App Bar</SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_VOICE_AI}>
                  Voice AI
                </SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_DYNAMIC}>
                  Dynamic
                </SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_ORB}>Orb (3D)</SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_PERSONA}>
                  Persona (AI Avatar)
                </SelectItem>
              </SelectContent>
            </Select>
          }
        />
        <SettingsRow
          htmlFor="sound-enabled"
          label="Sound effects"
          description="Play sounds for notifications and feedback"
          control={
            <Switch
              id="sound-enabled"
              checked={settings.soundEnabled}
              onCheckedChange={settings.handleSoundEnabledChange}
            />
          }
        />
      </SettingsGroup>

      <SettingsGroup title="Agent">
        <SettingsRow
          htmlFor="companion-mode"
          label="Companion mode"
          description={
            'Observe-only: Juno watches your screen and advises — like "What does this error mean?" — but never clicks, types, or acts'
          }
          control={
            <Switch
              id="companion-mode"
              checked={companionMode}
              onCheckedChange={handleCompanionModeChange}
              disabled={companionModeLoading}
            />
          }
        />
        <SettingsRow
          advanced
          htmlFor="agent-mode"
          label="Agent mode"
          description="Multi-agent uses specialized agents per task; single agent uses one agent for everything"
          control={
            <Select
              value={settings.agentMode}
              onValueChange={settings.handleAgentModeChange}
            >
              <SelectTrigger id="agent-mode" className="w-44">
                <SelectValue placeholder="Select agent mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="multi">Multi-Agent (Recommended)</SelectItem>
                <SelectItem value="single">Single Agent</SelectItem>
              </SelectContent>
            </Select>
          }
        />
        <SettingsRow
          advanced
          htmlFor="agent-trigger-mode"
          label="Trigger mode"
          description="Tap to toggle agent mode on/off, or hold the key to activate and release to stop"
          control={
            <Select
              value={settings.agentTriggerMode}
              onValueChange={settings.handleAgentTriggerModeChange}
            >
              <SelectTrigger id="agent-trigger-mode" className="w-44">
                <SelectValue placeholder="Select trigger mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tap">Tap to Toggle (Default)</SelectItem>
                <SelectItem value="hold">Hold to Activate</SelectItem>
              </SelectContent>
            </Select>
          }
        />
      </SettingsGroup>

      <SettingsGroup title="Cursor" advanced>
        <SettingsRow
          htmlFor="big-cursor-enabled"
          label="Big cursor"
          description="Enlarges the system cursor during agent execution so you can track what the agent is doing"
          control={
            <Switch
              id="big-cursor-enabled"
              checked={bigCursorEnabled}
              onCheckedChange={handleBigCursorEnabledChange}
              disabled={bigCursorLoading}
            />
          }
        >
          {systemCursorSize > 1.0 && (
            <div className="mb-3 flex items-center justify-between rounded-md border border-border bg-muted/50 p-3">
              <p className="text-sm text-muted-foreground">
                Cursor is currently enlarged ({systemCursorSize.toFixed(1)}x)
              </p>
              <Button
                variant="outline"
                size="sm"
                onClick={async () => {
                  try {
                    await invoke("test_cursor_restore");
                    const sysSize = await invoke<number>(
                      "get_system_cursor_size"
                    );
                    setSystemCursorSize(sysSize);
                    toast.success("Cursor restored to normal");
                  } catch (e) {
                    toast.error("Failed to restore cursor");
                  }
                }}
              >
                Reset to Normal
              </Button>
            </div>
          )}
          {bigCursorEnabled && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Cursor scale</span>
                <span className="tabular-nums text-sm text-muted-foreground">
                  {bigCursorScale.toFixed(1)}x
                </span>
              </div>
              <Slider
                value={[bigCursorScale]}
                onValueChange={handleBigCursorScaleChange}
                onValueCommit={handleBigCursorScaleCommit}
                min={1.5}
                max={10}
                step={0.5}
              />
              <p className="text-xs text-muted-foreground">
                How much larger to make the cursor (1.5x – 10x)
              </p>
              <div className="flex gap-2 pt-1">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={async () => {
                    try {
                      await invoke("test_cursor_scale", {
                        scale: bigCursorScale,
                      });
                      const sysSize = await invoke<number>(
                        "get_system_cursor_size"
                      );
                      setSystemCursorSize(sysSize);
                      toast.success(
                        `Cursor scaled to ${bigCursorScale.toFixed(1)}x`
                      );
                    } catch (e) {
                      toast.error("Failed to test cursor scale");
                    }
                  }}
                >
                  Test Scale
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={async () => {
                    try {
                      await invoke("test_cursor_restore");
                      const sysSize = await invoke<number>(
                        "get_system_cursor_size"
                      );
                      setSystemCursorSize(sysSize);
                      toast.success("Cursor restored to normal");
                    } catch (e) {
                      toast.error("Failed to restore cursor");
                    }
                  }}
                >
                  Restore
                </Button>
              </div>
            </div>
          )}
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Onboarding" advanced>
        <SettingsRow
          label="Restart onboarding"
          description={
            <>
              Go through the welcome guide and setup process again
              {onboardingInfo?.is_development_mode && (
                <span className="mt-1 block text-muted-foreground">
                  Development mode: Onboarding always shows on restart
                </span>
              )}
              {onboardingInfo?.completed_at && (
                <span className="mt-1 block text-muted-foreground/70">
                  Last completed:{" "}
                  {new Date(onboardingInfo.completed_at).toLocaleDateString()}
                </span>
              )}
            </>
          }
          control={
            <Button
              onClick={handleRestartOnboarding}
              disabled={restartOnboardingLoading}
              variant="outline"
              size="sm"
            >
              <RotateCcw
                className={`mr-2 h-4 w-4 ${
                  restartOnboardingLoading ? "animate-spin" : ""
                }`}
              />
              {restartOnboardingLoading ? "Restarting..." : "Restart"}
            </Button>
          }
        />
      </SettingsGroup>
    </>
  );
}
