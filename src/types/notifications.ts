export type NotificationType = "system" | "toast" | "both" | "disabled";

export interface NotificationSettings {
  type: NotificationType;
  sound_enabled: boolean;
  duration: number; // For toast notifications (in milliseconds)
  position: "top-left" | "top-right" | "bottom-left" | "bottom-right" | "top-center" | "bottom-center";
  show_icons: boolean;
  persist_important: boolean; // Keep important notifications until manually dismissed
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