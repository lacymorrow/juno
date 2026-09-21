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

export interface SystemNotificationPermission {
  granted: boolean;
  denied: boolean;
  default: boolean;
}