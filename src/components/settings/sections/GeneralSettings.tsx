import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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

export default function GeneralSettings({ settings }: SettingsSectionProps) {
  const [autoLaunchEnabled, setAutoLaunchEnabled] = useState(false);
  const [autoLaunchLoading, setAutoLaunchLoading] = useState(false);

  // Load auto-launch status on component mount
  useEffect(() => {
    const loadAutoLaunchStatus = async () => {
      try {
        const enabled = await invoke<boolean>("is_autostart_enabled");
        setAutoLaunchEnabled(enabled);
      } catch (error) {
        console.error("Failed to load auto-launch status:", error);
        // Default to false if unable to determine status
        setAutoLaunchEnabled(false);
      }
    };

    loadAutoLaunchStatus();
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

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          General Settings
        </h3>

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
                  <SelectItem value="multi">
                    Multi-Agent (Recommended)
                  </SelectItem>
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
      </div>
    </div>
  );
}
