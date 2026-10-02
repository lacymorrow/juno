import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
 * Rust reads what macOS currently allows for Juno and this draws it: ask when
 * macOS has never asked, point at System Settings when the person turned them
 * off, and a live switch only when macOS allows them. The status is read again
 * whenever the window regains focus, so coming back from System Settings
 * updates the row without a restart.
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

  // Returning from System Settings: read the answer again.
  useEffect(() => {
    const refresh = () => void readStatus();
    const onVisible = () => {
      if (document.visibilityState === "visible") refresh();
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [readStatus]);

  const change = async (next: boolean) => {
    const previous = enabled;
    setEnabled(next);
    setTestResult(null);
    try {
      await invoke(COMMANDS.NOTIFICATIONS_SET_NOTIFICATIONS_ENABLED, { enabled: next });
    } catch (error) {
      console.error("Failed to change notifications:", error);
      setEnabled(previous);
      toast.error(typeof error === "string" ? error : "Could not change that setting");
    }
  };

  const allow = async () => {
    try {
      setStatus(
        await invoke<NotificationStatus>(COMMANDS.NOTIFICATIONS_REQUEST_NOTIFICATION_PERMISSION),
      );
    } catch (error) {
      console.error("Failed to ask for notifications:", error);
      toast.error(typeof error === "string" ? error : "Could not ask for notifications");
    }
  };

  const openSystemSettings = async () => {
    try {
      await invoke(COMMANDS.NOTIFICATIONS_OPEN_NOTIFICATION_SETTINGS);
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

  const authorization = status?.authorization ?? null;
  const authorized = authorization === "authorized";

  let description = "Checking.";
  if (authorized) description = "Turn this off and Juno stays silent.";
  else if (authorization === "denied")
    description = "Notifications are off for Juno in System Settings.";
  else if (authorization === "not_determined")
    description = "Juno needs your OK to show notifications.";
  else if (authorization === "unavailable") {
    description = status?.unavailable_reason ?? "Notifications are not available here.";
  }

  return (
    <div className="space-y-6">
      <SettingsGroup title="Notifications">
        <SettingsRow
          htmlFor="notifications-enabled"
          label="Show notifications"
          description={description}
        >
          <div className="flex items-center gap-3">
            {authorization === "not_determined" && (
              <Button size="sm" onClick={() => void allow()}>
                Allow notifications
              </Button>
            )}
            {authorization === "denied" && (
              <Button size="sm" variant="outline" onClick={() => void openSystemSettings()}>
                Open System Settings
              </Button>
            )}
            <Switch
              id="notifications-enabled"
              checked={authorized && enabled}
              disabled={loading || !authorized}
              onCheckedChange={change}
            />
          </div>
        </SettingsRow>

        {authorized && (
          <SettingsRow
            label="Test"
            description="Send one now, so you know what to expect."
            below={
              testResult ? (
                <p className="text-[12px] leading-snug text-muted-foreground">
                  {testResult.ok ? "Sent." : testResult.reason}
                </p>
              ) : undefined
            }
          >
            <Button
              size="sm"
              variant="outline"
              disabled={testing || !enabled}
              onClick={() => void sendTest()}
            >
              {testing ? "Sending" : "Send one"}
            </Button>
          </SettingsRow>
        )}
      </SettingsGroup>
    </div>
  );
}

export default NotificationSettings;
