import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/constants.generated";
import { safeUnlistenAll } from "@/lib/tauri-event-utils";

interface MenuEventsProps {
	// Navigation
	setCurrentView: (view: "chat" | "devtools" | "permissions") => void;
	setIsDevPanelOpen: (value: boolean | ((current: boolean) => boolean)) => void;

	// Modal management
	setActiveModal: (modal: "help" | "feedback" | "export" | "import" | "update" | null) => void;
	setFeedbackData: (data: any) => void;

	// Chat actions
	startNewChat: () => void;

	// System message
	addSystemMessage: (message: string) => void;

	// Update check
	handleUpdateCheck: () => Promise<void>;
}

export function useMenuEvents({
	setCurrentView,
	setIsDevPanelOpen,
	setActiveModal,
	setFeedbackData,
	startNewChat,
	addSystemMessage,
	handleUpdateCheck,
}: MenuEventsProps) {
  useEffect(() => {
    let unlistenCallbacks: (() => void)[] = [];

    const setupMenuListeners = async () => {
      // View management events removed - orphaned (never emitted by backend)

      // Modal management events removed - orphaned (never emitted by backend)

      // Chat management events removed - orphaned (never emitted by backend)

      // Update management events removed - orphaned (never emitted by backend)

      // Application events removed - orphaned (never emitted by backend)

      // Window management events removed - orphaned (never emitted by backend)

      // Settings events removed - orphaned (never emitted by backend)

      // Developer tools - using generated constants
      unlistenCallbacks.push(
        await listen(EVENTS.MENU_DEVTOOLS_REQUESTED, () => {
          console.log("🔧 Menu: Opening developer tools");
          setCurrentView("devtools");
        }),
      );

      unlistenCallbacks.push(
        await listen(EVENTS.MENU_TOGGLE_DEV_PANEL_REQUESTED, () => {
          console.log("🔧 Menu: Toggling dev tools panel");
          setIsDevPanelOpen((current: boolean) => !current);
        }),
      );

      // Performance and debugging events removed - orphaned (never emitted by backend)

      // Edit operations are now handled natively by Tauri's PredefinedMenuItem
      // No frontend event listeners needed for copy, paste, cut, undo, redo, select all

      console.log(
        `✅ Menu events initialized with ${unlistenCallbacks.length} listeners`,
      );
    };

    setupMenuListeners().catch((error) => {
      console.error("❌ Failed to setup menu listeners:", error);
    });

		// Cleanup function with safe unlisten
		return () => {
			safeUnlistenAll(unlistenCallbacks);
			console.log("🧹 Menu event listeners cleaned up");
		};
	}, [
		setCurrentView,
		setIsDevPanelOpen,
		setActiveModal,
		setFeedbackData,
		startNewChat,
		addSystemMessage,
		handleUpdateCheck,
	]);
}

/**
 * Edit operations are now handled natively by Tauri's PredefinedMenuItem.
 * No custom JavaScript handlers needed.
 * This eliminates the context menu issue and provides native app behavior.
 */
