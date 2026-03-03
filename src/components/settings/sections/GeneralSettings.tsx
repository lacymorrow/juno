import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
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
import { RotateCcw, Sparkles } from "lucide-react";
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

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Startup Behavior</CardTitle>
          <CardDescription>
            Configure how Juno behaves when starting up
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label htmlFor="auto-launch" className="text-sm font-medium">
                Launch at Login
              </Label>
              <p className="text-xs text-gray-500">
                Automatically start Juno when you log in to your computer
              </p>
            </div>
            <Switch
              id="auto-launch"
              checked={autoLaunchEnabled}
              onCheckedChange={handleAutoLaunchChange}
              disabled={autoLaunchLoading}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-blue-500" />
            Onboarding
          </CardTitle>
          <CardDescription>
            Restart the onboarding flow to learn about Juno's features
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label className="text-sm font-medium">
                Restart Onboarding Flow
              </Label>
              <p className="text-xs text-gray-500">
                Go through the welcome guide and setup process again
                {onboardingInfo?.is_development_mode && (
                  <span className="block text-blue-600 mt-1">
                    Development mode: Onboarding always shows on restart
                  </span>
                )}
              </p>
              {onboardingInfo?.completed_at && (
                <p className="text-xs text-gray-400 mt-1">
                  Last completed:{" "}
                  {new Date(onboardingInfo.completed_at).toLocaleDateString()}
                </p>
              )}
            </div>
            <Button
              onClick={handleRestartOnboarding}
              disabled={restartOnboardingLoading}
              variant="outline"
              size="sm"
            >
              {restartOnboardingLoading ? (
                <>
                  <RotateCcw className="w-4 h-4 mr-2 animate-spin" />
                  Restarting...
                </>
              ) : (
                <>
                  <RotateCcw className="w-4 h-4 mr-2" />
                  Restart Onboarding
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Bar Appearance</CardTitle>
          <CardDescription>
            Choose which bar UI style to use in bar windows
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Label htmlFor="bar-appearance">Appearance</Label>
            <Select
              value={barAppearance}
              onValueChange={handleBarAppearanceChange}
              disabled={barAppearanceLoading}
            >
              <SelectTrigger>
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
                <SelectItem value={UI.BAR_APPEARANCES_ORB}>
                  Orb (3D)
                </SelectItem>
                <SelectItem value={UI.BAR_APPEARANCES_PERSONA}>
                  Persona (AI Avatar)
                </SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-gray-500">
              Bar windows will switch styles immediately when changed.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Sound Effects</CardTitle>
          <CardDescription>
            Configure audio feedback and notifications
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label htmlFor="sound-enabled" className="text-sm font-medium">
                Enable Sound Effects
              </Label>
              <p className="text-xs text-gray-500">
                Play sounds for notifications and feedback
              </p>
            </div>
            <Switch
              id="sound-enabled"
              checked={settings.soundEnabled}
              onCheckedChange={settings.handleSoundEnabledChange}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Agent Mode</CardTitle>
          <CardDescription>
            Choose how Juno handles tasks and AI interactions
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Label htmlFor="agent-mode">Agent Mode</Label>
            <Select
              value={settings.agentMode}
              onValueChange={settings.handleAgentModeChange}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select agent mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="multi">Multi-Agent (Recommended)</SelectItem>
                <SelectItem value="single">Single Agent</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-gray-500">
              Multi-agent mode uses specialized agents for different tasks,
              while single agent mode uses one agent for everything.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Agent Trigger Mode</CardTitle>
          <CardDescription>
            Choose how to activate the AI agent with the shortcut key
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Label htmlFor="agent-trigger-mode">Trigger Mode</Label>
            <Select
              value={settings.agentTriggerMode}
              onValueChange={settings.handleAgentTriggerModeChange}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select trigger mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tap">Tap to Toggle (Default)</SelectItem>
                <SelectItem value="hold">Hold to Activate</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-gray-500">
              <strong>Tap to Toggle:</strong> Press and release to toggle agent
              mode on/off.
              <br />
              <strong>Hold to Activate:</strong> Hold key to activate agent,
              release to stop (like dictation mode).
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
