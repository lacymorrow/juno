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
 * What macOS says about Juno's notifications (UNUserNotificationCenter), read
 * in Rust. `unavailable` means this process cannot notify as Juno at all, such
 * as a development build.
 */
export type NotificationAuthorization =
  | "authorized"
  | "denied"
  | "not_determined"
  | "unavailable";

/** Everything the notifications row draws. */
export interface NotificationStatus {
  authorization: NotificationAuthorization;
  /** Why nothing can be posted, when `authorization` is `unavailable`. */
  unavailable_reason: string | null;
}
