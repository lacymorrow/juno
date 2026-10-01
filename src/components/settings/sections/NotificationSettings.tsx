import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CircleAlert, CircleCheck } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import type {
  NotificationSettings as Settings,
  NotificationStatus,
} from "@/types/notifications";

import { SettingsGroup, SettingsRow } from "../ui";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Notifications.
 *
 * One switch, what macOS will do with it, and one button to try it. Every
 * sentence on this screen is written in Rust and rendered here, because
 * whether a notification can appear is a backend fact.
 *
 * The row this replaces read "Allowed" on every machine, including one where
 * no notification had appeared in weeks. It was drawn from the notification
 * plugin's permission check, which on desktop is a hard-coded `granted` that
 * asks macOS nothing. Beside it sat an "Ask" button wired to the plugin's
 * `request_permission`, which is the same hard-coded `granted` and could never
 * have asked anyone anything. Both are gone. What is left says how far Juno's
 * knowledge goes, and points at the one place a person can change the answer.
 */
export function NotificationSettings() {
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<NotificationStatus | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<
    { ok: true } | { ok: false; reason: string } | null
  >(null);

  const readStatus = useCallback(async () => {
    try {
      setStatus(
        await invoke<NotificationStatus>(
          COMMANDS.NOTIFICATIONS_CHECK_NOTIFICATION_PERMISSION,
        ),
      );
    } catch (error) {
      console.error("Failed to check notifications:", error);
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const settings = await invoke<Settings>(COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS);
        if (mounted) setEnabled(settings.enabled);
      } catch (error) {
        console.error("Failed to load notification settings:", error);
      } finally {
        if (mounted) setLoading(false);
      }
      await readStatus();
    })();
    return () => {
      mounted = false;
    };
  }, [readStatus]);

  const change = async (next: boolean) => {
    const previous = enabled;
    setEnabled(next);
    setTestResult(null);
    try {
      await invoke(COMMANDS.NOTIFICATIONS_SET_NOTIFICATIONS_ENABLED, { enabled: next });
      // The switch is one of the four things that decide whether a
      // notification can appear, so the row below it is re-read, not guessed.
      await readStatus();
    } catch (error) {
      console.error("Failed to change notifications:", error);
      setEnabled(previous);
      toast.error(typeof error === "string" ? error : "Could not change that setting");
    }
  };

  const openSystemSettings = async (pane: string) => {
    try {
      await invoke(COMMANDS.PERMISSIONS_OPEN_SYSTEM_SETTINGS, { permission_type: pane });
    } catch (error) {
      console.error("Failed to open System Settings:", error);
      toast.error("Could not open System Settings");
    }
  };

  const sendTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      await invoke(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);
      setTestResult({ ok: true });
    } catch (error) {
      setTestResult({
        ok: false,
        reason:
          typeof error === "string" ? error : "Juno could not send a test notification.",
      });
    } finally {
      setTesting(false);
    }
  };

  const StatusIcon = status?.can_notify ? CircleCheck : CircleAlert;

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Notifications"
        footer="Juno notifies you when an automation runs, when the agent schedules one, when a background session finishes or needs you, and when it needs the real pointer. macOS decides how those look and how long they stay."
      >
        <SettingsRow
          htmlFor="notifications-enabled"
          label="Show notifications"
          description="Turn this off and Juno stays silent."
        >
          <Switch
            id="notifications-enabled"
            checked={enabled}
            disabled={loading}
            onCheckedChange={change}
          />
        </SettingsRow>

        <SettingsRow
          label="macOS notifications"
          description={status ? status.detail : "Checking."}
        >
          {status && (
            <div className="flex items-center gap-3">
              <span className="flex items-center gap-1.5 text-[13px] text-muted-foreground">
                <StatusIcon className="size-3.5 shrink-0" aria-hidden="true" />
                {status.headline}
              </span>
              {status.system_settings_pane && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    const pane = status.system_settings_pane;
                    if (pane) void openSystemSettings(pane);
                  }}
                >
                  Open Settings
                </Button>
              )}
            </div>
          )}
        </SettingsRow>

        <SettingsRow
          label="Test"
          description="Send one now, so you know what to expect."
          below={
            testResult ? (
              <p className="text-[12px] leading-snug text-muted-foreground">
                {testResult.ok
                  ? "Sent to macOS. If nothing appeared, macOS held it back: check System Settings > Notifications > Juno."
                  : testResult.reason}
              </p>
            ) : undefined
          }
        >
          <Button
            size="sm"
            variant="outline"
            disabled={testing || !status || !status.can_notify}
            onClick={() => void sendTest()}
          >
            {testing ? "Sending" : "Send one"}
          </Button>
        </SettingsRow>
      </SettingsGroup>
    </div>
  );
}

export default NotificationSettings;
