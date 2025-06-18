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

    // Copy/save operations
    copyingMessageId: string | null;
    savingMessageId: string | null;

    // UI state
    userHasScrolledUp: boolean;
    lastScrollTime: number;

    // App metadata
    appVersion: string | null;
    keyboardShortcuts: {
        agent_mode_toggle: string;
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

    // Copy and save operation state
    const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
    const [savingMessageId, setSavingMessageId] = useState<string | null>(null);

    // Scroll management
    const [userHasScrolledUp, setUserHasScrolledUp] = useState(false);
    const [lastScrollTime, setLastScrollTime] = useState(0);

    // App metadata
    const [appVersion, setAppVersion] = useState<string | null>(null);
    const [keyboardShortcuts, setKeyboardShortcuts] = useState<{
        agent_mode_toggle: string;
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

    const resetCopyingState = useCallback(() => {
        setTimeout(() => setCopyingMessageId(null), 1000);
    }, []);

    const resetSavingState = useCallback(() => {
        setTimeout(() => setSavingMessageId(null), 1000);
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
        copyingMessageId,
        savingMessageId,
        userHasScrolledUp,
        lastScrollTime,
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
        setCopyingMessageId,
        setSavingMessageId,
        resetCopyingState,
        resetSavingState,
        setUserHasScrolledUp,
        setLastScrollTime,
        setAppVersion,
        setKeyboardShortcuts,
    };
}
