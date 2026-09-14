import { useState, useCallback } from "react";
import type { AppView } from "@/components/AppHeader";
import type { ModalType, FeedbackData, UpdateInfo } from "@/components/ModalSystem";

export interface AppState {
    // View management
    currentView: AppView;
    isDevPanelOpen: boolean;

    // Processing state
    isProcessing: boolean;
    serverStatus: "connected" | "error" | "connecting";

    // Modal state
    activeModal: ModalType;
    feedbackData: FeedbackData;
    updateInfo: UpdateInfo | null;
    isCheckingUpdate: boolean;

    // Which assistant message just got copied (its button shows a check)
    copiedMessageId: string | null;

    // Voice/Agent state
    isAgentModeActive: boolean;
    isDictationActive: boolean;
    dictationState: string;

    // App metadata
    appVersion: string | null;
    keyboardShortcuts: {
        agent_mode: string;
        dictation_input: string;
        stop_current_task: string;
        open_settings: string;
    } | null;
}

const initialFeedbackData: FeedbackData = {
    type: "general",
    title: "",
    description: "",
    email: "",
    priority: "medium",
};

export function useAppState() {
    const [currentView, setCurrentView] = useState<AppView>("chat");
    const [isDevPanelOpen, setIsDevPanelOpen] = useState(false);
    const [isProcessing, setIsProcessing] = useState(false);
    const [serverStatus, setServerStatus] = useState<"connected" | "error" | "connecting">("connecting");

    // Modal state
    const [activeModal, setActiveModal] = useState<ModalType>(null);
    const [feedbackData, setFeedbackData] = useState<FeedbackData>(initialFeedbackData);
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);

    // Copy feedback state (the reset timer lives in useConversation)
    const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);

    // Voice/Agent state
    const [isAgentModeActive, setIsAgentModeActive] = useState(false);
    const [isDictationActive, setIsDictationActive] = useState(false);
    const [dictationState, setDictationState] = useState("idle");

    // App metadata
    const [appVersion, setAppVersion] = useState<string | null>(null);
    const [keyboardShortcuts, setKeyboardShortcuts] = useState<{
        agent_mode: string;
        dictation_input: string;
        stop_current_task: string;
        open_settings: string;
    } | null>(null);

    // Derived state
    const canSubmit = !isProcessing && serverStatus === "connected";

    // Action creators
    const toggleDevPanel = useCallback(() => {
        setIsDevPanelOpen(prev => !prev);
    }, []);

    const handleFeedbackDataChange = useCallback((data: Partial<FeedbackData>) => {
        setFeedbackData(prev => ({ ...prev, ...data }));
    }, []);

    return {
        // State
        currentView,
        isDevPanelOpen,
        isProcessing,
        serverStatus,
        activeModal,
        feedbackData,
        updateInfo,
        isCheckingUpdate,
        copiedMessageId,
        isAgentModeActive,
        isDictationActive,
        dictationState,
        appVersion,
        keyboardShortcuts,
        canSubmit,

        // Actions
        setCurrentView,
        setIsDevPanelOpen,
        toggleDevPanel,
        setIsProcessing,
        setServerStatus,
        setActiveModal,
        setFeedbackData,
        handleFeedbackDataChange,
        setUpdateInfo,
        setIsCheckingUpdate,
        setCopiedMessageId,
        setIsAgentModeActive,
        setIsDictationActive,
        setDictationState,
        setAppVersion,
        setKeyboardShortcuts,
    };
}
