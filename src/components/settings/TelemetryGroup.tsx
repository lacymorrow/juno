import { Switch } from "@/components/ui/switch";
import { SettingsGroup, SettingsRow } from "./ui";
import { TELEMETRY_SUMMARY } from "./telemetrySummary";
import { useTelemetrySetting } from "./useTelemetrySetting";

export function TelemetryGroup() {
  const { enabled, saving, change } = useTelemetrySetting();
  return (
    <SettingsGroup title="Privacy">
      <SettingsRow
        htmlFor="share-usage-data"
        label="Share anonymous usage data"
        description={TELEMETRY_SUMMARY}
      >
        <Switch
          id="share-usage-data"
          checked={enabled}
          disabled={saving}
          onCheckedChange={(checked) => void change(checked)}
        />
      </SettingsRow>
    </SettingsGroup>
  );
}
