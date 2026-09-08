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
        footer="Multi-agent mode uses specialized agents for different tasks; single-agent mode uses one agent for everything. Tap to toggle agent mode on and off, or hold to activate it while the key is held (like dictation)."
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
        <SettingsRow
          htmlFor="agent-trigger-mode"
          label="Trigger mode"
          description="How the shortcut key activates the agent"
        >
          <Select
            value={settings.agentTriggerMode}
            onValueChange={settings.handleAgentTriggerModeChange}
          >
            <SelectTrigger id="agent-trigger-mode" className="w-[190px]">
              <SelectValue placeholder="Select trigger mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="tap">Tap to Toggle (Default)</SelectItem>
              <SelectItem value="hold">Hold to Activate</SelectItem>
            </SelectContent>
          </Select>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Companion Mode"
        advanced
        footer='Observe-only mode: Juno watches your screen and advises without clicking, typing, or taking any actions. Ask things like "What does this error mean?" — Juno describes and advises but never acts.'
      >
        <SettingsRow
          htmlFor="companion-mode"
          label="Enable Companion Mode"
          description="Juno advises but never controls the computer"
        >
          <Switch
            id="companion-mode"
            checked={companionMode}
            onCheckedChange={handleCompanionModeChange}
            disabled={companionModeLoading}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Big Cursor" advanced>
        {systemCursorSize > 1.0 && (
          <SettingsRow
            label="Cursor is currently enlarged"
            description={`Scaled to ${systemCursorSize.toFixed(1)}x`}
          >
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  await invoke("test_cursor_restore");
                  const sysSize = await invoke<number>("get_system_cursor_size");
                  setSystemCursorSize(sysSize);
                  toast.success("Cursor restored to normal");
                } catch (e) {
                  toast.error("Failed to restore cursor");
                }
              }}
            >
              Reset to Normal
            </Button>
          </SettingsRow>
        )}
        <SettingsRow
          htmlFor="big-cursor-enabled"
          label="Enable big cursor"
          description="Enlarge the system cursor while the agent controls your computer, so it is easy to track"
        >
          <Switch
            id="big-cursor-enabled"
            checked={bigCursorEnabled}
            onCheckedChange={handleBigCursorEnabledChange}
            disabled={bigCursorLoading}
          />
        </SettingsRow>
        {bigCursorEnabled && (
          <SettingsRow
            label="Cursor scale"
            description="How much larger to make the cursor (1.5x – 10x)"
            below={
              <div className="space-y-3">
                <div className="flex items-center gap-3">
                  <Slider
                    value={[bigCursorScale]}
                    onValueChange={handleBigCursorScaleChange}
                    onValueCommit={handleBigCursorScaleCommit}
                    min={1.5}
                    max={10}
                    step={0.5}
                    className="flex-1"
                  />
                  <span className="w-10 shrink-0 text-right text-[13px] tabular-nums text-muted-foreground">
                    {bigCursorScale.toFixed(1)}x
                  </span>
                </div>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={async () => {
                      try {
                        await invoke("test_cursor_scale", { scale: bigCursorScale });
                        const sysSize = await invoke<number>("get_system_cursor_size");
                        setSystemCursorSize(sysSize);
                        toast.success(`Cursor scaled to ${bigCursorScale.toFixed(1)}x`);
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
                        const sysSize = await invoke<number>("get_system_cursor_size");
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
            }
          />
        )}
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
              <SelectItem value={UI.BAR_APPEARANCES_ORB}>Orb (3D)</SelectItem>
              <SelectItem value={UI.BAR_APPEARANCES_PERSONA}>
                Persona (AI Avatar)
              </SelectItem>
            </SelectContent>
          </Select>
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
    </div>
  );
}
