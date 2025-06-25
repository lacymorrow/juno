import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/constants.generated";

interface MenuEventsProps {
	// Navigation
	setCurrentView: (view: "chat" | "devtools" | "permissions") => void;
	setIsDevPanelOpen: (value: boolean | ((current: boolean) => boolean)) => void;

	// Modal management
	setActiveModal: (modal: "help" | "feedback" | "export" | "import" | "update" | null) => void;
	setFeedbackData: (data: any) => void;

	// Chat actions
	startNewChat: () => void;
	clearConversation: () => void;

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
	clearConversation,
	addSystemMessage,
	handleUpdateCheck,
}: MenuEventsProps) {
	useEffect(() => {
		let unlistenCallbacks: (() => void)[] = [];

		const setupMenuListeners = async () => {
			// View management
			unlistenCallbacks.push(
				await listen("menu-view-chat", () => {
					console.log("📱 Menu: Switching to chat view");
					setCurrentView("chat");
				})
			);

			unlistenCallbacks.push(
				await listen("menu-view-devtools", () => {
					console.log("🛠️ Menu: Switching to devtools view");
					setCurrentView("devtools");
				})
			);

			unlistenCallbacks.push(
				await listen("menu-view-permissions", () => {
					console.log("🔒 Menu: Switching to permissions view");
					setCurrentView("permissions");
				})
			);

			// Modal management
			unlistenCallbacks.push(
				await listen("menu-show-help", () => {
					console.log("❓ Menu: Opening help modal");
					setActiveModal("help");
				})
			);

			unlistenCallbacks.push(
				await listen("menu-show-feedback", () => {
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
				await listen("menu-export-chat", () => {
					console.log("📤 Menu: Opening export modal");
					setActiveModal("export");
				})
			);

			unlistenCallbacks.push(
				await listen("menu-import-chat", () => {
					console.log("📥 Menu: Opening import modal");
					setActiveModal("import");
				})
			);

			// Chat management
			unlistenCallbacks.push(
				await listen("menu-new-chat", () => {
					console.log("🆕 Menu: Starting new chat");
					startNewChat();
					addSystemMessage("🆕 New chat started");
				})
			);

			unlistenCallbacks.push(
				await listen("menu-clear-chat", () => {
					console.log("🗑️ Menu: Clearing chat");
					clearConversation();
					addSystemMessage("🗑️ Chat history cleared");
				})
			);

			// Update management
			unlistenCallbacks.push(
				await listen("menu-check-updates", () => {
					console.log("🔄 Menu: Checking for updates");
					handleUpdateCheck();
				})
			);

			// Application events
			unlistenCallbacks.push(
				await listen("app-ready", () => {
					console.log("🚀 App ready event received");
				})
			);

			unlistenCallbacks.push(
				await listen("app-focus", () => {
					console.log("👀 App focus event received");
				})
			);

			unlistenCallbacks.push(
				await listen("app-blur", () => {
					console.log("😴 App blur event received");
				})
			);

			// Window management
			unlistenCallbacks.push(
				await listen("window-minimize", () => {
					console.log("⬇️ Window minimize requested");
				})
			);

			unlistenCallbacks.push(
				await listen("window-maximize", () => {
					console.log("⬆️ Window maximize requested");
				})
			);

			unlistenCallbacks.push(
				await listen("window-close", () => {
					console.log("❌ Window close requested");
				})
			);

			// Settings and configuration
			unlistenCallbacks.push(
				await listen("menu-open-settings", () => {
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
				await listen("menu-reload-app", () => {
					console.log("🔄 Menu: Reloading application");
					window.location.reload();
				})
			);

			unlistenCallbacks.push(
				await listen("menu-force-reload", () => {
					console.log("🔄 Menu: Force reloading application");
					window.location.reload();
				})
			);

			// Accessibility and user assistance
			unlistenCallbacks.push(
				await listen("menu-zoom-in", () => {
					console.log("🔍 Menu: Zoom in requested");
					// Zoom functionality could be implemented here
				})
			);

			unlistenCallbacks.push(
				await listen("menu-zoom-out", () => {
					console.log("🔍 Menu: Zoom out requested");
					// Zoom functionality could be implemented here
				})
			);

			unlistenCallbacks.push(
				await listen("menu-reset-zoom", () => {
					console.log("🔍 Menu: Reset zoom requested");
					// Reset zoom functionality could be implemented here
				})
			);

			// === Edit Menu Events ===
			unlistenCallbacks.push(
				await listen('menu-edit-undo', () => {
					console.log("↶ Menu: Undo requested");
					handleEditUndo();
				})
			);

			unlistenCallbacks.push(
				await listen('menu-edit-redo', () => {
					console.log("↷ Menu: Redo requested");
					handleEditRedo();
				})
			);

			unlistenCallbacks.push(
				await listen('menu-edit-cut', () => {
					console.log("✂️ Menu: Cut requested");
					handleEditCut();
				})
			);

			unlistenCallbacks.push(
				await listen('menu-edit-copy', () => {
					console.log("📋 Menu: Copy requested");
					handleEditCopy();
				})
			);

			unlistenCallbacks.push(
				await listen('menu-edit-paste', () => {
					console.log("📄 Menu: Paste requested");
					handleEditPaste();
				})
			);

			unlistenCallbacks.push(
				await listen('menu-edit-select-all', () => {
					console.log("🔘 Menu: Select All requested");
					handleEditSelectAll();
				})
			);

			console.log(`✅ Menu events initialized with ${unlistenCallbacks.length} listeners`);
		};

		setupMenuListeners().catch((error) => {
			console.error("❌ Failed to setup menu listeners:", error);
		});

		// Cleanup function
		return () => {
			unlistenCallbacks.forEach((unlisten) => {
				try {
					unlisten();
				} catch (error) {
					console.error("❌ Error cleaning up menu listener:", error);
				}
			});
			console.log("🧹 Menu event listeners cleaned up");
		};
	}, [
		setCurrentView,
		setIsDevPanelOpen,
		setActiveModal,
		setFeedbackData,
		startNewChat,
		clearConversation,
		addSystemMessage,
		handleUpdateCheck,
	]);
}

/**
 * Handle Edit menu actions by delegating to the focused element
 */

function handleEditUndo() {
	console.log('[Menu] Handling undo');

	// Try to execute undo command on the focused element
	if (document.activeElement) {
		const activeElement = document.activeElement as HTMLElement;

		// For input elements, try to trigger undo
		if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
			// Try to use execCommand (deprecated but still works for basic operations)
			try {
				document.execCommand('undo');
			} catch (error) {
				console.warn('[Menu] Unable to execute undo:', error);
			}
		}
	}

	// Try keyboard shortcut as fallback
	simulateKeyboardShortcut('z', true);
}

function handleEditRedo() {
	console.log('[Menu] Handling redo');

	// Try to execute redo command on the focused element
	if (document.activeElement) {
		const activeElement = document.activeElement as HTMLElement;

		// For input elements, try to trigger redo
		if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
			try {
				document.execCommand('redo');
			} catch (error) {
				console.warn('[Menu] Unable to execute redo:', error);
			}
		}
	}

	// Try keyboard shortcut as fallback
	simulateKeyboardShortcut('z', true, true); // Cmd+Shift+Z
}

function handleEditCut() {
	console.log('[Menu] Handling cut');

	// Try to cut selected text
	if (document.activeElement) {
		const activeElement = document.activeElement as HTMLInputElement | HTMLTextAreaElement;

		if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
			if (activeElement.selectionStart !== null && activeElement.selectionEnd !== null) {
				const selectedText = activeElement.value.substring(
					activeElement.selectionStart,
					activeElement.selectionEnd
				);

				if (selectedText) {
					// Copy to clipboard
					navigator.clipboard.writeText(selectedText).then(() => {
						// Remove selected text
						const newValue =
							activeElement.value.substring(0, activeElement.selectionStart!) +
							activeElement.value.substring(activeElement.selectionEnd!);

						activeElement.value = newValue;
						activeElement.dispatchEvent(new Event('input', { bubbles: true }));
					});
					return;
				}
			}
		}
	}

	// Try using execCommand as fallback
	try {
		document.execCommand('cut');
	} catch (error) {
		console.warn('[Menu] Unable to execute cut:', error);
		// Fallback to keyboard shortcut
		simulateKeyboardShortcut('x', true);
	}
}

function handleEditCopy() {
	console.log('[Menu] Handling copy');

	// Try to copy selected text
	if (document.activeElement) {
		const activeElement = document.activeElement as HTMLInputElement | HTMLTextAreaElement;

		if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
			if (activeElement.selectionStart !== null && activeElement.selectionEnd !== null) {
				const selectedText = activeElement.value.substring(
					activeElement.selectionStart,
					activeElement.selectionEnd
				);

				if (selectedText) {
					navigator.clipboard.writeText(selectedText);
					return;
				}
			}
		}
	}

	// Try to copy any selected text on the page
	const selection = window.getSelection();
	if (selection && selection.toString()) {
		navigator.clipboard.writeText(selection.toString());
		return;
	}

	// Try using execCommand as fallback
	try {
		document.execCommand('copy');
	} catch (error) {
		console.warn('[Menu] Unable to execute copy:', error);
		// Fallback to keyboard shortcut
		simulateKeyboardShortcut('c', true);
	}
}

function handleEditPaste() {
	console.log('[Menu] Handling paste');

	// Try to paste from clipboard
	navigator.clipboard.readText().then(text => {
		if (document.activeElement) {
			const activeElement = document.activeElement as HTMLInputElement | HTMLTextAreaElement;

			if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
				if (activeElement.selectionStart !== null && activeElement.selectionEnd !== null) {
					// Insert text at cursor position
					const newValue =
						activeElement.value.substring(0, activeElement.selectionStart) +
						text +
						activeElement.value.substring(activeElement.selectionEnd);

					activeElement.value = newValue;
					activeElement.dispatchEvent(new Event('input', { bubbles: true }));
					return;
				}
			}
		}

		// Try using execCommand as fallback
		try {
			document.execCommand('insertText', false, text);
		} catch (error) {
			console.warn('[Menu] Unable to execute paste:', error);
			// Fallback to keyboard shortcut
			simulateKeyboardShortcut('v', true);
		}
	}).catch(error => {
		console.warn('[Menu] Unable to read clipboard:', error);
		// Fallback to keyboard shortcut
		simulateKeyboardShortcut('v', true);
	});
}

function handleEditSelectAll() {
	console.log('[Menu] Handling select all');

	// Try to select all text in focused element
	if (document.activeElement) {
		const activeElement = document.activeElement as HTMLInputElement | HTMLTextAreaElement;

		if (activeElement.tagName === 'INPUT' || activeElement.tagName === 'TEXTAREA') {
			activeElement.select();
			return;
		}
	}

	// Try to select all text on the page
	try {
		const selection = window.getSelection();
		if (selection) {
			selection.selectAllChildren(document.body);
		}
	} catch (error) {
		console.warn('[Menu] Unable to select all:', error);
		// Fallback to keyboard shortcut
		simulateKeyboardShortcut('a', true);
	}
}

/**
 * Simulate keyboard shortcut as fallback
 */
function simulateKeyboardShortcut(key: string, cmdOrCtrl: boolean = false, shift: boolean = false) {
	const event = new KeyboardEvent('keydown', {
		key: key,
		code: `Key${key.toUpperCase()}`,
		metaKey: cmdOrCtrl && navigator.platform.includes('Mac'),
		ctrlKey: cmdOrCtrl && !navigator.platform.includes('Mac'),
		shiftKey: shift,
		bubbles: true,
		cancelable: true
	});

	document.dispatchEvent(event);
}
