import { Switch } from "@/components/ui/switch";
import { TELEMETRY_SUMMARY } from "@/components/settings/telemetrySummary";
import { useTelemetrySetting } from "@/components/settings/useTelemetrySetting";

/**
 * Onboarding's one privacy question. The same switch Settings shows, saved
 * through the same hook. Default on; Continue with no choice keeps it on.
 */
export function PrivacyStep() {
  const { enabled, saving, change } = useTelemetrySetting();
  return (
    <div className="pt-4 text-left">
      <div className="rounded-xl border border-border bg-card p-4">
        <p className="text-[13px] leading-snug text-muted-foreground">{TELEMETRY_SUMMARY}</p>
        <label
          htmlFor="onboarding-share-usage-data"
          className="mt-4 flex items-center justify-between gap-3 text-[14px] font-medium"
        >
          Share anonymous usage data
          <Switch
            id="onboarding-share-usage-data"
            checked={enabled}
            disabled={saving}
            onCheckedChange={(checked) => void change(checked)}
          />
        </label>
      </div>
      <p className="mt-3 text-[12px] text-muted-foreground">Change this any time in Settings.</p>
    </div>
  );
}
