import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SettingsSectionProps } from "./types";
import { SettingsGroup, SettingsRow } from "./ui";

/** Whether Juno splits a task across specialized agents or uses one. */
export function AgentModeGroup({ settings }: SettingsSectionProps) {
  return (
    <SettingsGroup
      title="Agent"
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
            <SelectItem value="single">Single Agent</SelectItem>
            <SelectItem value="multi">Multi-Agent</SelectItem>
          </SelectContent>
        </Select>
      </SettingsRow>
    </SettingsGroup>
  );
}
