import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

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
                    console.log("�� Menu: Starting new chat");
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

            // Developer tools
            unlistenCallbacks.push(
                await listen("menu-toggle-devtools", () => {
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
