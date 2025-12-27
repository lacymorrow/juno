import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NotificationData, NotificationSettings } from "@/types/notifications";
import { 
  CheckCircle, 
  AlertCircle, 
  AlertTriangle, 
  Info,
  Bell
} from "lucide-react";
import React from "react";

export class NotificationService {
  private static instance: NotificationService;
  private isInitialized = false;

  private constructor() {}

  public static getInstance(): NotificationService {
    if (!NotificationService.instance) {
      NotificationService.instance = new NotificationService();
    }
    return NotificationService.instance;
  }

  public async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }

    try {
      // Listen for toast notification events from the backend
      await listen<any>("show-toast-notification", (event) => {
        this.showToastNotification(event.payload);
      });

      this.isInitialized = true;
      console.log("NotificationService initialized");
    } catch (error) {
      console.error("Failed to initialize NotificationService:", error);
    }
  }

  private showToastNotification(data: any): void {
    const {
      title,
      message,
      level,
      important,
      timeout,
      show_icons,
      persist_important
    } = data;

    // Determine the toast function based on level
    const toastFunction = this.getToastFunction(level);

    // Get icon if icons are enabled
    const icon = show_icons ? this.getIconForLevel(level) : undefined;

    // Determine duration
    const duration = important && persist_important ? Infinity : (timeout || 5000);

    // Create a unique ID based on title and message to prevent duplicates
    const toastId = `backend-${level}-${(title || '').slice(0, 30)}-${(message || '').slice(0, 30)}`.replace(/\s+/g, '-');

    // Dismiss any existing toast with the same ID before showing new one
    toast.dismiss(toastId);

    // Show the toast with deduplication ID
    toastFunction(title, {
      id: toastId,
      description: message,
      duration: duration,
      action: important ? {
        label: "Dismiss",
        onClick: () => {},
      } : undefined,
      icon: icon,
    });
  }

  private getToastFunction(level: string) {
    switch (level) {
      case "success":
        return toast.success;
      case "error":
        return toast.error;
      case "warning":
        return toast.warning;
      case "info":
      default:
        return toast.info;
    }
  }

  private getIconForLevel(level: string) {
    const iconProps = { className: "w-5 h-5" };
    
    switch (level) {
      case "success":
        return React.createElement(CheckCircle, iconProps);
      case "error":
        return React.createElement(AlertCircle, iconProps);
      case "warning":
        return React.createElement(AlertTriangle, iconProps);
      case "info":
        return React.createElement(Info, iconProps);
      default:
        return React.createElement(Bell, iconProps);
    }
  }

  // Public method to send notifications
  public async sendNotification(notification: NotificationData): Promise<void> {
    try {
      await invoke("send_notification", {
        notificationData: {
          title: notification.title,
          message: notification.message,
          level: notification.level,
          important: notification.important || false,
          icon: notification.icon || null,
          timeout: notification.timeout || null,
        }
      });
    } catch (error) {
      console.error("Failed to send notification:", error);
      // Fallback to local toast if backend fails
      toast.error("Failed to send notification");
    }
  }

  // Convenience methods for different notification types
  public async info(title: string, message: string, options?: Partial<NotificationData>): Promise<void> {
    await this.sendNotification({
      title,
      message,
      level: "info",
      ...options
    });
  }

  public async success(title: string, message: string, options?: Partial<NotificationData>): Promise<void> {
    await this.sendNotification({
      title,
      message,
      level: "success",
      ...options
    });
  }

  public async warning(title: string, message: string, options?: Partial<NotificationData>): Promise<void> {
    await this.sendNotification({
      title,
      message,
      level: "warning",
      ...options
    });
  }

  public async error(title: string, message: string, options?: Partial<NotificationData>): Promise<void> {
    await this.sendNotification({
      title,
      message,
      level: "error",
      ...options
    });
  }

  // Method to get current notification settings
  public async getSettings(): Promise<NotificationSettings> {
    try {
      const settings = await invoke<{
        notification_type: string;
        sound_enabled: boolean;
        duration: number;
        position: string;
        show_icons: boolean;
        persist_important: boolean;
      }>("get_notification_settings");

      return {
        type: settings.notification_type as any,
        sound_enabled: settings.sound_enabled,
        duration: settings.duration,
        position: settings.position as any,
        show_icons: settings.show_icons,
        persist_important: settings.persist_important,
      };
    } catch (error) {
      console.error("Failed to get notification settings:", error);
      throw error;
    }
  }

  // Method to test notification system
  public async testNotification(): Promise<void> {
    try {
      await invoke("test_notification");
    } catch (error) {
      console.error("Failed to test notification:", error);
      throw error;
    }
  }
}

// Export singleton instance
export const notificationService = NotificationService.getInstance();