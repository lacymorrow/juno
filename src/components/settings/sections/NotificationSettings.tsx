import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import type {
  NotificationSettings as Settings,
  SystemNotificationPermission,
} from "@/types/notifications";

import { SettingsGroup, SettingsRow } from "../ui";

/**
 * Notifications.
 *
 * One switch and the macOS permission behind it. This screen used to offer six
 * controls: notification type, sound, duration, position, show icons, and
 * persist important. None of them did anything. They were stored, read back by
 * this screen, and consulted by nothing on the way to an actual notification,
 * and four of them could not have worked in any case, because macOS decides how
 * a user notification is presented. "Disabled" was the worst of them: it
 * promised in as many words that Juno would stop, and Juno carried on.
 *
 * What is left is the choice a person actually wants and Juno can actually
 * keep.
 */
export function NotificationSettings() {
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [permission, setPermission] = useState<SystemNotificationPermission>({
    granted: false,
    denied: false,
    default: true,
  });
  const [requesting, setRequesting] = useState(false);

  const readPermission = useCallback(async () => {
    try {
      setPermission(
        await invoke<SystemNotificationPermission>(
          "check_notification_permission",
        ),
      );
    } catch (error) {
      console.error("Failed to check notification permission:", error);
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const settings = await invoke<Settings>("get_notification_settings");
        if (mounted) setEnabled(settings.enabled);
      } catch (error) {
        console.error("Failed to load notification settings:", error);
      } finally {
        if (mounted) setLoading(false);
      }
      await readPermission();
    })();
    return () => {
      mounted = false;
    };
  }, [readPermission]);

  const change = async (next: boolean) => {
    const previous = enabled;
    setEnabled(next);
    try {
      await invoke("set_notifications_enabled", { enabled: next });
    } catch (error) {
      console.error("Failed to change notifications:", error);
      setEnabled(previous);
      toast.error(
        typeof error === "string" ? error : "Could not change that setting",
      );
    }
  };

  const requestPermission = async () => {
    setRequesting(true);
    try {
      const result = await invoke<SystemNotificationPermission>(
        "request_notification_permission",
      );
      setPermission(result);
      if (result.denied) {
        toast.error(
          "macOS denied the request. You can still allow Juno in System Settings > Notifications.",
        );
      }
    } catch (error) {
      console.error("Failed to request notification permission:", error);
      toast.error("Could not ask macOS for permission");
    } finally {
      setRequesting(false);
    }
  };

  const status = permission.granted ? (
    <Badge variant="secondary">Allowed</Badge>
  ) : permission.denied ? (
    <Badge variant="destructive">Denied</Badge>
  ) : (
    <Badge variant="outline">Not asked</Badge>
  );

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Notifications"
        footer="Juno notifies you when an automation runs, when the agent schedules one, and when it needs the real pointer. macOS decides how those look and how long they stay."
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
          label="macOS permission"
          description={
            permission.granted
              ? "Juno can show notifications."
              : permission.denied
                ? "Denied. Allow Juno in System Settings > Notifications."
                : "Not requested yet."
          }
        >
          <div className="flex items-center gap-2">
            {status}
            {!permission.granted && !permission.denied && (
              <Button
                onClick={requestPermission}
                disabled={requesting}
                size="sm"
                variant="outline"
              >
                {requesting ? "Asking..." : "Ask"}
              </Button>
            )}
          </div>
        </SettingsRow>

        <SettingsRow
          label="Test"
          description="Send one now, so you know what to expect."
        >
          <Button
            size="sm"
            variant="outline"
            disabled={!enabled}
            onClick={() => {
              void invoke("test_notification").catch((error) => {
                console.error("Failed to send test notification:", error);
                toast.error("Could not send a test notification");
              });
            }}
          >
            Send one
          </Button>
        </SettingsRow>
      </SettingsGroup>
    </div>
  );
}

export default NotificationSettings;
