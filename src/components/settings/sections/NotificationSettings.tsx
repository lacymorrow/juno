import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  Bell,
  BellOff,
  Volume2,
  VolumeX,
  TestTube,
  Shield,
  Info,
} from "lucide-react";
import {
  NotificationSettings as NotificationSettingsType,
  NotificationType,
  SystemNotificationPermission,
} from "@/types/notifications";

export default function NotificationSettings() {
  const [notificationSettings, setNotificationSettings] =
    useState<NotificationSettingsType>({
      type: "system",
      sound_enabled: true,
      duration: 5000,
      position: "bottom-right",
      show_icons: true,
      persist_important: true,
    });

  const [systemPermission, setSystemPermission] =
    useState<SystemNotificationPermission>({
      granted: false,
      denied: false,
      default: true,
    });

  const [loading, setLoading] = useState(false);
  const [permissionLoading, setPermissionLoading] = useState(false);

  // Load notification settings on component mount
  useEffect(() => {
    loadNotificationSettings();
    checkSystemPermission();
  }, []);

  const loadNotificationSettings = async () => {
    try {
      const current = await invoke<{
        notification_type: string;
        sound_enabled: boolean;
        duration: number;
        position: string;
        show_icons: boolean;
        persist_important: boolean;
      }>("get_notification_settings");

      setNotificationSettings({
        type: current.notification_type as NotificationType,
        sound_enabled: current.sound_enabled,
        duration: current.duration,
        position: current.position as any,
        show_icons: current.show_icons,
        persist_important: current.persist_important,
      });
    } catch (error) {
      console.error("Failed to load notification settings:", error);
      toast.error("Failed to load notification settings");
    }
  };

  const checkSystemPermission = async () => {
    try {
      const permission = await invoke<SystemNotificationPermission>(
        "check_notification_permission"
      );
      setSystemPermission(permission);
    } catch (error) {
      console.error("Failed to check notification permission:", error);
    }
  };

  const requestSystemPermission = async () => {
    setPermissionLoading(true);
    try {
      const permission = await invoke<SystemNotificationPermission>(
        "request_notification_permission"
      );
      setSystemPermission(permission);

      if (permission.granted) {
        toast.success("Notification permission granted!");
      } else if (permission.denied) {
        toast.error(
          "Notification permission denied. You can enable it in system settings."
        );
      }
    } catch (error) {
      console.error("Failed to request notification permission:", error);
      toast.error("Failed to request notification permission");
    } finally {
      setPermissionLoading(false);
    }
  };

  const updateNotificationType = async (type: NotificationType) => {
    try {
      await invoke("set_notification_type", { notificationType: type });
      setNotificationSettings((prev) => ({ ...prev, type }));
      toast.success(`Notification type set to: ${type}`);
    } catch (error) {
      console.error("Failed to update notification type:", error);
      toast.error("Failed to update notification type");
    }
  };

  const updateSoundEnabled = async (enabled: boolean) => {
    try {
      await invoke("set_notification_sound_enabled", { enabled });
      setNotificationSettings((prev) => ({ ...prev, sound_enabled: enabled }));
      toast.success(`Notification sound ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update notification sound:", error);
      toast.error("Failed to update notification sound");
    }
  };

  const updateDuration = async (duration: number) => {
    try {
      await invoke("set_notification_duration", { duration });
      setNotificationSettings((prev) => ({ ...prev, duration }));
    } catch (error) {
      console.error("Failed to update notification duration:", error);
      toast.error("Failed to update notification duration");
    }
  };

  const updatePosition = async (position: string) => {
    try {
      await invoke("set_notification_position", { position });
      setNotificationSettings((prev) => ({
        ...prev,
        position: position as any,
      }));
      toast.success(`Notification position set to: ${position}`);
    } catch (error) {
      console.error("Failed to update notification position:", error);
      toast.error("Failed to update notification position");
    }
  };

  const updateShowIcons = async (showIcons: boolean) => {
    try {
      await invoke("set_notification_show_icons", { showIcons });
      setNotificationSettings((prev) => ({ ...prev, show_icons: showIcons }));
      toast.success(`Notification icons ${showIcons ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update notification icons:", error);
      toast.error("Failed to update notification icons");
    }
  };

  const updatePersistImportant = async (persist: boolean) => {
    try {
      await invoke("set_notification_persist_important", { persist });
      setNotificationSettings((prev) => ({
        ...prev,
        persist_important: persist,
      }));
      toast.success(
        `Important notification persistence ${persist ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to update notification persistence:", error);
      toast.error("Failed to update notification persistence");
    }
  };

  const testNotification = async () => {
    setLoading(true);
    try {
      await invoke("test_notification");
      // Only show toast confirmation if toast notifications are enabled
      if (
        notificationSettings.type === "toast" ||
        notificationSettings.type === "both"
      ) {
        toast.success("Test notification sent!");
      }
    } catch (error) {
      console.error("Failed to send test notification:", error);
      // Always show error toasts regardless of settings for debugging purposes
      toast.error("Failed to send test notification");
    } finally {
      setLoading(false);
    }
  };

  const getPermissionStatusBadge = () => {
    if (systemPermission.granted) {
      return (
        <Badge variant="default" className="bg-green-100 text-green-800">
          Granted
        </Badge>
      );
    } else if (systemPermission.denied) {
      return <Badge variant="destructive">Denied</Badge>;
    } else {
      return <Badge variant="secondary">Not Requested</Badge>;
    }
  };

  const getTypeDescription = (type: NotificationType) => {
    switch (type) {
      case "system":
        return "Show notifications using the operating system's notification center";
      case "toast":
        return "Show notifications as in-app toast messages";
      case "both":
        return "Show both system notifications and in-app toast messages";
      case "disabled":
        return "Disable all notifications";
      default:
        return "";
    }
  };

  return (
    <div className="space-y-6">
      {/* Notification Type */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bell className="w-5 h-5" />
            Notification Type
          </CardTitle>
          <CardDescription>
            Choose how you want to receive notifications from Juno
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-3">
            <Label htmlFor="notification-type">Notification Method</Label>
            <Select
              value={notificationSettings.type}
              onValueChange={(value) =>
                updateNotificationType(value as NotificationType)
              }
            >
              <SelectTrigger id="notification-type">
                <SelectValue placeholder="Select notification type" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="system">System Notifications</SelectItem>
                <SelectItem value="toast">Toast Notifications</SelectItem>
                <SelectItem value="both">Both System & Toast</SelectItem>
                <SelectItem value="disabled">Disabled</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-sm text-gray-600">
              {getTypeDescription(notificationSettings.type)}
            </p>
          </div>

          {/* Test Notification Button */}
          <div className="pt-4">
            <Button
              onClick={testNotification}
              disabled={loading || notificationSettings.type === "disabled"}
              variant="outline"
              className="flex items-center gap-2"
            >
              <TestTube className="w-4 h-4" />
              {loading ? "Sending..." : "Test Notification"}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* System Notification Permission */}
      {(notificationSettings.type === "system" ||
        notificationSettings.type === "both") && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="w-5 h-5" />
              System Notification Permission
            </CardTitle>
            <CardDescription>
              System notifications require permission from macOS
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-1">
                <p className="text-sm font-medium">Permission Status</p>
                <p className="text-sm text-gray-600">
                  {systemPermission.granted
                    ? "Juno can show system notifications"
                    : systemPermission.denied
                    ? "Permission denied. Enable in System Settings > Notifications"
                    : "Permission not yet requested"}
                </p>
              </div>
              <div className="flex items-center gap-2">
                {getPermissionStatusBadge()}
                {!systemPermission.granted && (
                  <Button
                    onClick={requestSystemPermission}
                    disabled={permissionLoading}
                    size="sm"
                    variant="outline"
                  >
                    {permissionLoading ? "Requesting..." : "Request Permission"}
                  </Button>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Sound Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            {notificationSettings.sound_enabled ? (
              <Volume2 className="w-5 h-5" />
            ) : (
              <VolumeX className="w-5 h-5" />
            )}
            Sound Settings
          </CardTitle>
          <CardDescription>
            Configure notification sound preferences
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <Label htmlFor="sound-enabled">Notification Sounds</Label>
              <p className="text-sm text-gray-600">
                Play a sound when notifications are shown
              </p>
            </div>
            <Switch
              id="sound-enabled"
              checked={notificationSettings.sound_enabled}
              onCheckedChange={updateSoundEnabled}
            />
          </div>
        </CardContent>
      </Card>

      {/* Toast Notification Settings */}
      {(notificationSettings.type === "toast" ||
        notificationSettings.type === "both") && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Info className="w-5 h-5" />
              Toast Notification Settings
            </CardTitle>
            <CardDescription>
              Configure in-app toast notification behavior
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {/* Duration */}
            <div className="space-y-3">
              <Label htmlFor="duration">
                Duration: {notificationSettings.duration / 1000}s
              </Label>
              <Slider
                id="duration"
                min={1000}
                max={15000}
                step={500}
                value={[notificationSettings.duration]}
                onValueChange={([value]: number[]) => updateDuration(value)}
                className="w-full"
              />
              <p className="text-sm text-gray-600">
                How long toast notifications stay visible (1-15 seconds)
              </p>
            </div>

            {/* Position */}
            <div className="space-y-3">
              <Label htmlFor="position">Position</Label>
              <Select
                value={notificationSettings.position}
                onValueChange={updatePosition}
              >
                <SelectTrigger id="position">
                  <SelectValue placeholder="Select position" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="top-left">Top Left</SelectItem>
                  <SelectItem value="top-center">Top Center</SelectItem>
                  <SelectItem value="top-right">Top Right</SelectItem>
                  <SelectItem value="bottom-left">Bottom Left</SelectItem>
                  <SelectItem value="bottom-center">Bottom Center</SelectItem>
                  <SelectItem value="bottom-right">Bottom Right</SelectItem>
                </SelectContent>
              </Select>
              <p className="text-sm text-gray-600">
                Where toast notifications appear on screen
              </p>
            </div>

            {/* Show Icons */}
            <div className="flex items-center justify-between">
              <div className="space-y-1">
                <Label htmlFor="show-icons">Show Icons</Label>
                <p className="text-sm text-gray-600">
                  Display icons in toast notifications
                </p>
              </div>
              <Switch
                id="show-icons"
                checked={notificationSettings.show_icons}
                onCheckedChange={updateShowIcons}
              />
            </div>

            {/* Persist Important */}
            <div className="flex items-center justify-between">
              <div className="space-y-1">
                <Label htmlFor="persist-important">
                  Persist Important Notifications
                </Label>
                <p className="text-sm text-gray-600">
                  Keep important notifications until manually dismissed
                </p>
              </div>
              <Switch
                id="persist-important"
                checked={notificationSettings.persist_important}
                onCheckedChange={updatePersistImportant}
              />
            </div>
          </CardContent>
        </Card>
      )}

      {/* Disabled State Info */}
      {notificationSettings.type === "disabled" && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <BellOff className="w-5 h-5" />
              Notifications Disabled
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-gray-600">
              All notifications are currently disabled. Juno will not show any
              notifications for agent actions, completions, or errors. You can
              re-enable them by selecting a different notification type above.
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
