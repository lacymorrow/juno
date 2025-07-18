import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

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
			// View management
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_VIEW_CHAT, () => {
					console.log("📱 Menu: Switching to chat view");
					setCurrentView("chat");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_VIEW_DEVTOOLS, () => {
					console.log("🛠️ Menu: Switching to devtools view");
					setCurrentView("devtools");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_VIEW_PERMISSIONS, () => {
					console.log("🔒 Menu: Switching to permissions view");
					setCurrentView("permissions");
				})
			);

			// Modal management
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_SHOW_HELP, () => {
					console.log("❓ Menu: Opening help modal");
					setActiveModal("help");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_SHOW_FEEDBACK, () => {
					console.log("📝 Menu: Opening feedback modal");
					setFeedbackData({
						type: "general",
						title: "",
						description: "",
						email: "",
						priority: "medium",
					});
					setActiveModal("feedback");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_EXPORT_CHAT, () => {
					console.log("📤 Menu: Opening export modal");
					setActiveModal("export");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_IMPORT_CHAT, () => {
					console.log("📥 Menu: Opening import modal");
					setActiveModal("import");
				})
			);

			// Chat management
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_NEW_CHAT_REQUESTED, () => {
					console.log("🆕 Menu: Starting new chat");
					startNewChat();
					addSystemMessage("🆕 New chat started");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_CLEAR_CHAT, () => {
					console.log("🗑️ Menu: Clearing chat (using new chat)");
					startNewChat();
					addSystemMessage("🗑️ Chat history cleared");
				})
			);

			// Update management
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_UPDATE_CHECK_REQUESTED, () => {
					console.log("🔄 Menu: Checking for updates");
					handleUpdateCheck();
				})
			);

			// Application events
			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_APP_READY, () => {
					console.log("🚀 App ready event received");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_APP_FOCUS, () => {
					console.log("👀 App focus event received");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_APP_BLUR, () => {
					console.log("😴 App blur event received");
				})
			);

			// Window management
			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_WINDOW_MINIMIZE, () => {
					console.log("⬇️ Window minimize requested");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_WINDOW_MAXIMIZE, () => {
					console.log("⬆️ Window maximize requested");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.SYSTEM_WINDOW_CLOSE, () => {
					console.log("❌ Window close requested");
				})
			);

			// Settings and configuration
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_OPEN_SETTINGS, () => {
					console.log("⚙️ Menu: Opening settings");
					// Note: Settings opens in a separate window via backend
				})
			);

			// Developer tools - using generated constants
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_DEVTOOLS_REQUESTED, () => {
					console.log("🔧 Menu: Opening developer tools");
					setCurrentView("devtools");
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_TOGGLE_DEV_PANEL_REQUESTED, () => {
					console.log("🔧 Menu: Toggling dev tools panel");
					setIsDevPanelOpen((current: boolean) => !current);
				})
			);

			// Performance and debugging
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_RELOAD_APP, () => {
					console.log("🔄 Menu: Reloading application");
					window.location.reload();
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_FORCE_RELOAD, () => {
					console.log("🔄 Menu: Force reloading application");
					window.location.reload();
				})
			);

			// Accessibility and user assistance
			unlistenCallbacks.push(
				await listen(EVENTS.MENU_ZOOM_IN, () => {
					console.log("🔍 Menu: Zoom in requested");
					// Zoom functionality could be implemented here
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_ZOOM_OUT, () => {
					console.log("🔍 Menu: Zoom out requested");
					// Zoom functionality could be implemented here
				})
			);

			unlistenCallbacks.push(
				await listen(EVENTS.MENU_RESET_ZOOM, () => {
					console.log("🔍 Menu: Reset zoom requested");
					// Reset zoom functionality could be implemented here
				})
			);

			// Edit operations are now handled natively by Tauri's PredefinedMenuItem
			// No frontend event listeners needed for copy, paste, cut, undo, redo, select all

			console.log(`✅ Menu events initialized with ${unlistenCallbacks.length} listeners`);
		};

		setupMenuListeners().catch((error) => {
			console.error("❌ Failed to setup menu listeners:", error);
		});

		// Cleanup function
		return () => {
			unlistenCallbacks.forEach((unlisten) => {
				safeCleanupEventListener(unlisten);
			});
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
