export interface NotificationSettings {
  /** Whether Juno shows notifications at all. */
  enabled: boolean;
}

export interface NotificationData {
  title: string;
  message: string;
  level: "info" | "success" | "warning" | "error";
  important?: boolean;
  actions?: NotificationAction[];
  icon?: string;
  timeout?: number; // Override default duration
}

export interface NotificationAction {
  label: string;
  action: () => void;
  style?: "primary" | "secondary" | "destructive";
}

/**
 * What the notification plugin reports about permission, as a variant.
 *
 * `unknown` is its own answer, not a third guess. Worth knowing what this is
 * and is not: on desktop the plugin's permission check is a hard-coded
 * `granted` that asks macOS nothing, so this is never evidence that a banner
 * will appear.
 */
export type PluginPermission = "granted" | "denied" | "must_ask" | "unknown";

/** Whether a notification Juno posts can reach the screen. Rust decides. */
export type NotificationAvailability =
  | "macos_decides"
  | "off_in_juno"
  | "dev_build"
  | "unbundled";

/** Everything the notifications pane draws. Every word of it comes from Rust. */
export interface NotificationStatus {
  availability: NotificationAvailability;
  plugin_permission: PluginPermission;
  /** Short line for the right edge of the row. */
  headline: string;
  /** The sentence under the label: why, or how far Juno's knowledge goes. */
  detail: string;
  can_notify: boolean;
  /** The System Settings pane to open, when opening one would help. */
  system_settings_pane: string | null;
}
