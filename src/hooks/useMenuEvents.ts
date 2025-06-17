import { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toggleDictation } from "tauri-plugin-voice-transcription-api";
import type { AppView } from "@/components/AppHeader";
import type { ModalType, FeedbackData } from "@/components/ModalSystem";

interface UseMenuEventsProps {
    // Navigation
    setCurrentView: (view: AppView) => void;
    setIsDevPanelOpen: (open: boolean) => void;

    // Modal management
    setActiveModal: (modal: ModalType) => void;
    setFeedbackData: (data: React.SetStateAction<FeedbackData>) => void;

    // Chat actions
    startNewChat: () => void;
    clearConversation: () => void;

    // System message
    addSystemMessage: (content: string) => void;

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
}: UseMenuEventsProps) {

    // Listen for onboarding completion events from the onboarding window
    useEffect(() => {
        const unlistenComplete = listen<{ prompt?: string }>(
            "onboarding-complete",
            async (_event) => {
                console.log("Onboarding completed from separate window");
                const currentWindow = getCurrentWindow();
                await currentWindow.show();
                await currentWindow.setFocus();
            }
        );

        const unlistenSkipped = listen<{}>("onboarding-skipped", async (_event) => {
            console.log("Onboarding skipped from separate window");
            const currentWindow = getCurrentWindow();
            await currentWindow.show();
            await currentWindow.setFocus();
        });

        return () => {
            unlistenComplete.then((fn) => fn());
            unlistenSkipped.then((fn) => fn());
        };
    }, []);

    // Listen for settings menu requests from native menu
    useEffect(() => {
        const unlisten = listen<string>("settings-requested", async (event) => {
            console.log("Settings requested from menu:", event.payload);
            try {
                await invoke("open_settings_window");
            } catch (error) {
                console.error("Failed to open settings window:", error);
            }
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, []);

    // Listen for devtools menu requests from tray menu
    useEffect(() => {
        const unlisten = listen<string>("devtools-requested", (event) => {
            console.log("DevTools requested from tray menu:", event.payload);
            setCurrentView("devtools");
            setIsDevPanelOpen(true);
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [setCurrentView, setIsDevPanelOpen]);

    // Enhanced help request handler
    useEffect(() => {
        const unlisten = listen<string>("help-requested", async (event) => {
            console.log("Help requested from menu:", event.payload);
            const helpType = event.payload;

            if (helpType === "shortcuts") {
                try {
                    await invoke("open_settings_window");
                } catch (error) {
                    console.error("Failed to open settings window:", error);
                }
            } else {
                setActiveModal("help");
            }
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [setActiveModal]);

    // Listen for new chat requests
    useEffect(() => {
        const unlisten = listen("new-chat-requested", () => {
            console.log("New chat requested from menu");
            startNewChat();
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [startNewChat]);

    // Listen for clear history requests
    useEffect(() => {
        const unlisten = listen("clear-history-requested", () => {
            console.log("Clear history requested from menu");
            clearConversation();
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [clearConversation]);

    // Listen for toggle floating bar requests
    useEffect(() => {
        const unlisten = listen("toggle-floating-bar-requested", () => {
            console.log("Toggle floating bar requested from menu");
            // Floating bar is managed by backend, just log for now
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, []);

    // Listen for toggle dev panel requests
    useEffect(() => {
        const unlisten = listen("toggle-dev-panel-requested", () => {
            console.log("Toggle dev panel requested from menu");
            setIsDevPanelOpen((current) => !current);
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [setIsDevPanelOpen]);

    // Listen for permissions requests
    useEffect(() => {
        const unlisten = listen("permissions-requested", () => {
            console.log("Permissions requested from menu");
            setCurrentView("permissions");
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [setCurrentView]);

    // Enhanced feedback request handler
    useEffect(() => {
        const unlisten = listen<string>("feedback-requested", (event) => {
            console.log("Feedback requested from menu:", event.payload);
            const feedbackType = event.payload;

            setFeedbackData((prev) => ({
                ...prev,
                type: feedbackType === "issue" ? "issue" : "general",
            }));
            setActiveModal("feedback");
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [setFeedbackData, setActiveModal]);

    // Enhanced import/export chat handlers
    useEffect(() => {
        const unlistenImport = listen("import-chat-requested", () => {
            console.log("Import chat requested from menu");
            setActiveModal("import");
        });

        const unlistenExport = listen("export-chat-requested", () => {
            console.log("Export chat requested from menu");
            setActiveModal("export");
        });

        return () => {
            unlistenImport.then((unlistenFn) => unlistenFn());
            unlistenExport.then((unlistenFn) => unlistenFn());
        };
    }, [setActiveModal]);

    // Enhanced window management handlers
    useEffect(() => {
        const unlistenMinimize = listen("minimize-window-requested", async () => {
            console.log("Minimize window requested from menu");
            try {
                const window = getCurrentWindow();
                await window.minimize();
                console.log("✅ Window minimized successfully");
            } catch (error) {
                console.error("❌ Failed to minimize window:", error);
                addSystemMessage(`Failed to minimize window: ${error}`);
            }
        });

        const unlistenZoom = listen("zoom-window-requested", async () => {
            console.log("Zoom window requested from menu");
            try {
                const window = getCurrentWindow();
                const isMaximized = await window.isMaximized();
                if (isMaximized) {
                    await window.unmaximize();
                    console.log("✅ Window unmaximized successfully");
                } else {
                    await window.maximize();
                    console.log("✅ Window maximized successfully");
                }
            } catch (error) {
                console.error("❌ Failed to toggle window zoom:", error);
                addSystemMessage(`Failed to toggle window zoom: ${error}`);
            }
        });

        const unlistenFullscreen = listen("toggle-fullscreen-requested", async () => {
            console.log("Toggle fullscreen requested from menu");
            try {
                const window = getCurrentWindow();
                const isFullscreen = await window.isFullscreen();
                await window.setFullscreen(!isFullscreen);
                console.log(`✅ Window fullscreen ${!isFullscreen ? "enabled" : "disabled"} successfully`);
            } catch (error) {
                console.error("❌ Failed to toggle fullscreen:", error);
                addSystemMessage(`Failed to toggle fullscreen: ${error}`);
            }
        });

        const unlistenUpdate = listen("update-check-requested", () => {
            console.log("Update check requested from menu");
            handleUpdateCheck();
        });

        return () => {
            unlistenMinimize.then((unlistenFn) => unlistenFn());
            unlistenZoom.then((unlistenFn) => unlistenFn());
            unlistenFullscreen.then((unlistenFn) => unlistenFn());
            unlistenUpdate.then((unlistenFn) => unlistenFn());
        };
    }, [addSystemMessage, handleUpdateCheck]);

    // Listen for dictation toggle requests
    useEffect(() => {
        const unlisten = listen("toggle-dictation-request", async () => {
            console.log("Received toggle-dictation-request event");
            try {
                const isNowDictating = await toggleDictation();
                console.log("Toggled dictation, now dictating:", isNowDictating);
            } catch (error) {
                console.error("Failed to toggle dictation:", error);
                addSystemMessage(`Failed to toggle dictation: ${error}`);
            }
        });

        return () => {
            unlisten.then((unlistenFn) => unlistenFn());
        };
    }, [addSystemMessage]);
}
