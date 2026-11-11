import { useCallback, useEffect, useState } from "react";
import { isTelemetryEnabled, setTelemetryEnabled } from "@/lib/telemetry";

/** The one telemetry switch, shared by Settings and onboarding. */
export function useTelemetrySetting() {
  const [enabled, setEnabled] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let mounted = true;
    void isTelemetryEnabled().then((value) => {
      if (mounted) setEnabled(value);
    });
    return () => {
      mounted = false;
    };
  }, []);

  const change = useCallback(async (next: boolean) => {
    setEnabled(next);
    setSaving(true);
    try {
      await setTelemetryEnabled(next);
    } catch {
      setEnabled(!next);
    } finally {
      setSaving(false);
    }
  }, []);

  return { enabled, saving, change };
}
