import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { MessageSquarePlus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type {
  NotificationSettings as Settings,
  NotificationStatus,
} from "@/types/notifications";

import { SettingsGroup, SettingsRow } from "./ui";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Notifications, one row in General.
 *
 * Rust reads what macOS currently allows for Juno and this draws it: ask when
 * macOS has never asked, point at System Settings when the person turned them
 * off, and a live switch only when macOS allows them. The status is read again
 * whenever the window regains focus, so coming back from System Settings
 * updates the row without a restart.
 *
 * The sample sits beside the switch as one small icon, not a row of its own.
 * The banner is the answer, so there is no result line. A failure is logged
 * by Rust, and the status read afterwards says what to change.
 */
export function NotificationsGroup() {
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<NotificationStatus | null>(null);
  const [sending, setSending] = useState(false);

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
        const settings = await invoke<Settings>(
          COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS,
        );
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

  const sendSample = async () => {
    setSending(true);
    try {
      await invoke(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);
    } catch (error) {
      console.warn("Sample notification failed:", error);
    } finally {
      setSending(false);
    }
    // The first send may have asked macOS; show whatever it now says.
    await readStatus();
  };

  const change = async (next: boolean) => {
    const previous = enabled;
    setEnabled(next);
    try {
      await invoke(COMMANDS.NOTIFICATIONS_SET_NOTIFICATIONS_ENABLED, {
        enabled: next,
      });
    } catch (error) {
      console.error("Failed to change notifications:", error);
      setEnabled(previous);
      toast.error(
        typeof error === "string" ? error : "Could not change that setting",
      );
    }
  };

  const allow = async () => {
    try {
      const answer = await invoke<NotificationStatus>(
        COMMANDS.NOTIFICATIONS_REQUEST_NOTIFICATION_PERMISSION,
      );
      setStatus(answer);
    } catch (error) {
      console.warn("Failed to ask for notifications:", error);
      await readStatus();
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

  const authorization = status?.authorization ?? null;
  const authorized = authorization === "authorized";

  let description = "Allow Juno to display notifications on your computer.";
  if (authorization === "denied")
    description = "Turn on notifications for Juno in System Settings.";
  else if (authorization === "unavailable") {
    description =
      status?.unavailable_reason ??
      "Notifications aren't available on this Mac.";
  }

  return (
    <SettingsGroup title="Notifications">
      <SettingsRow
        id="show-notifications"
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
            <Button
              size="sm"
              variant="outline"
              onClick={() => void openSystemSettings()}
            >
              Open System Settings
            </Button>
          )}
          {authorized && enabled && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={sending}
                    onClick={() => void sendSample()}
                    aria-label="Send test notification"
                    className="text-muted-foreground hover:text-foreground"
                  >
                    <MessageSquarePlus className="size-4" aria-hidden="true" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top" className="text-[12px]">
                  Send test notification
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          <Switch
            id="notifications-enabled"
            checked={authorized && enabled}
            disabled={loading || !authorized}
            onCheckedChange={change}
          />
        </div>
      </SettingsRow>
    </SettingsGroup>
  );
}

export default NotificationsGroup;
