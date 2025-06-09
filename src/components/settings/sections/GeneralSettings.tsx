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
import { SettingsSectionProps } from "../types";

export default function GeneralSettings({ settings }: SettingsSectionProps) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          General Settings
        </h3>

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