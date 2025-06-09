import "../styles/globals.css"; // Make sure global styles are available
import SettingsWindow from "./settings/SettingsWindow";
import { emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export default function SettingsRoute() {
  const handleNavigateToDevTools = async () => {
    try {
      // First close settings window, then emit devtools request
      await invoke("close_settings_window");
      await emit("devtools-requested", "from-settings");
    } catch (error) {
      console.error("Failed to navigate to dev tools:", error);
    }
  };

  const handleNavigateToChat = async () => {
    try {
      // Close settings and show main window
      await invoke("close_settings_window");
      await emit("show-main-window");
    } catch (error) {
      console.error("Failed to navigate to chat:", error);
    }
  };

  const handleNavigateToPermissions = async () => {
    try {
      // Close settings and emit permissions request
      await invoke("close_settings_window");
      await emit("permissions-requested", "from-settings");
    } catch (error) {
      console.error("Failed to navigate to permissions:", error);
    }
  };

  return (
    <div className="h-screen w-screen bg-background text-foreground">
      <SettingsWindow
        onNavigateToDevTools={handleNavigateToDevTools}
        onNavigateToChat={handleNavigateToChat}
        onNavigateToPermissions={handleNavigateToPermissions}
      />
    </div>
  );
}
