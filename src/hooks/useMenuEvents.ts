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
		let mounted = true;
		let unlistenCallbacks: (() => void)[] = [];

		const addListener = async (event: string, handler: () => void) => {
			const unlisten = await listen(event, () => {
				if (mounted) handler();
			});
			if (mounted) unlistenCallbacks.push(unlisten);
			else safeCleanupEventListener(unlisten);
		};

		const setupMenuListeners = async () => {
			// View management
			await addListener(EVENTS.MENU_VIEW_CHAT, () => {
				console.log("📱 Menu: Switching to chat view");
				setCurrentView("chat");
			});

			await addListener(EVENTS.MENU_VIEW_DEVTOOLS, () => {
				console.log("🛠️ Menu: Switching to devtools view");
				setCurrentView("devtools");
			});

			await addListener(EVENTS.MENU_VIEW_PERMISSIONS, () => {
				console.log("🔒 Menu: Switching to permissions view");
				setCurrentView("permissions");
			});

			// Modal management
			await addListener(EVENTS.MENU_SHOW_HELP, () => {
				console.log("❓ Menu: Opening help modal");
				setActiveModal("help");
			});

			await addListener(EVENTS.MENU_SHOW_FEEDBACK, () => {
				console.log("📝 Menu: Opening feedback modal");
				setFeedbackData({
					type: "general",
					title: "",
					description: "",
					email: "",
					priority: "medium",
				});
				setActiveModal("feedback");
			});

			await addListener(EVENTS.MENU_EXPORT_CHAT, () => {
				console.log("📤 Menu: Opening export modal");
				setActiveModal("export");
			});

			await addListener(EVENTS.MENU_IMPORT_CHAT, () => {
				console.log("📥 Menu: Opening import modal");
				setActiveModal("import");
			});

			// Chat management
			await addListener(EVENTS.MENU_NEW_CHAT_REQUESTED, () => {
				console.log("🆕 Menu: Starting new chat");
				startNewChat();
				addSystemMessage("🆕 New chat started");
			});

			await addListener(EVENTS.MENU_CLEAR_CHAT, () => {
				console.log("🗑️ Menu: Clearing chat (using new chat)");
				startNewChat();
				addSystemMessage("🗑️ Chat history cleared");
			});

			// Update management
			await addListener(EVENTS.MENU_UPDATE_CHECK_REQUESTED, () => {
				console.log("🔄 Menu: Checking for updates");
				handleUpdateCheck();
			});

			// Application events
			await addListener(EVENTS.SYSTEM_APP_READY, () => {
				console.log("🚀 App ready event received");
			});

			await addListener(EVENTS.SYSTEM_APP_FOCUS, () => {
				console.log("👀 App focus event received");
			});

			await addListener(EVENTS.SYSTEM_APP_BLUR, () => {
				console.log("😴 App blur event received");
			});

			// Window management
			await addListener(EVENTS.SYSTEM_WINDOW_MINIMIZE, () => {
				console.log("⬇️ Window minimize requested");
			});

			await addListener(EVENTS.SYSTEM_WINDOW_MAXIMIZE, () => {
				console.log("⬆️ Window maximize requested");
			});

			await addListener(EVENTS.SYSTEM_WINDOW_CLOSE, () => {
				console.log("❌ Window close requested");
			});

			// Settings and configuration
			await addListener(EVENTS.MENU_OPEN_SETTINGS, () => {
				console.log("⚙️ Menu: Opening settings");
				// Note: Settings opens in a separate window via backend
			});

			// Developer tools - using generated constants
			await addListener(EVENTS.MENU_DEVTOOLS_REQUESTED, () => {
				console.log("🔧 Menu: Opening developer tools");
				setCurrentView("devtools");
			});

			await addListener(EVENTS.MENU_TOGGLE_DEV_PANEL_REQUESTED, () => {
				console.log("🔧 Menu: Toggling dev tools panel");
				setIsDevPanelOpen((current: boolean) => !current);
			});

			// Performance and debugging
			await addListener(EVENTS.MENU_RELOAD_APP, () => {
				console.log("🔄 Menu: Reloading application");
				window.location.reload();
			});

			await addListener(EVENTS.MENU_FORCE_RELOAD, () => {
				console.log("🔄 Menu: Force reloading application");
				window.location.reload();
			});

			// Accessibility and user assistance
			await addListener(EVENTS.MENU_ZOOM_IN, () => {
				console.log("🔍 Menu: Zoom in requested");
			});

			await addListener(EVENTS.MENU_ZOOM_OUT, () => {
				console.log("🔍 Menu: Zoom out requested");
			});

			await addListener(EVENTS.MENU_RESET_ZOOM, () => {
				console.log("🔍 Menu: Reset zoom requested");
			});

			// Edit operations are now handled natively by Tauri's PredefinedMenuItem
			// No frontend event listeners needed for copy, paste, cut, undo, redo, select all

			console.log(`✅ Menu events initialized with ${unlistenCallbacks.length} listeners`);
		};

		setupMenuListeners().catch((error) => {
			console.error("❌ Failed to setup menu listeners:", error);
		});

		// Cleanup function
		return () => {
			mounted = false;
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
