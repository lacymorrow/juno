import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import type { 
  ChatMessage, 
  AppView, 
  ModalType, 
  FeedbackData, 
  UpdateInfo,
  BackendResponsePayload
} from "@/types/chat";
import { debounce } from "@/lib/utils/chat";
import { playAudioFromBase64 } from "@/services/chatService";

export const useAppState = () => {
  // Core state
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [serverStatus, setServerStatus] = useState<"checking" | "connected" | "error">("checking");
  const [currentView, setCurrentView] = useState<AppView>("chat");
  const [appVersion, setAppVersion] = useState<string>("");
  const conversationEndRef = useRef<HTMLDivElement>(null);
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(null);

  // UI state
  const [isDevPanelOpen, setIsDevPanelOpen] = useState(false);
  const [activeModal, setActiveModal] = useState<ModalType>(null);
  const [feedbackData, setFeedbackData] = useState<FeedbackData>({
    type: "general",
    title: "",
    description: "",
    email: "",
    priority: "medium",
  });

  // Permissions and onboarding state
  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [onboardingChecked, setOnboardingChecked] = useState(false);

  // Operation state
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
  const [savingMessageId, setSavingMessageId] = useState<string | null>(null);

  // Fetch app version on mount
  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const version = await getVersion();
        setAppVersion(`v${version}`);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setAppVersion("v0.0.0");
      }
    };
    fetchVersion();
  }, []);

  // Backend response handler
  const handleBackendResponse = useCallback(
    debounce((payload: BackendResponsePayload) => {
      console.log("Debounced handler executing for:", payload.query);
      const { response } = payload;

      setConversation((prevConversation) => {
        const hasStreamingMessage = prevConversation.some(
          (msg: ChatMessage) => msg.isStreaming && msg.role === "assistant"
        );

        const now = Date.now();
        const isRecentlyStreamed = prevConversation.some(
          (msg: ChatMessage) =>
            msg.role === "assistant" &&
            msg.content === response.text &&
            msg.timestamp &&
            now - msg.timestamp < 2000
        );

        if (!hasStreamingMessage && !isRecentlyStreamed) {
          console.log("Adding assistant message from backend response");
          const assistantMessage: ChatMessage = {
            role: "assistant",
            content: response.text,
            isJsx: response.text.includes("<") && response.text.includes(">"),
            screenshot_base64: response.screenshot_base64,
            timestamp: Date.now(),
          };

          if (response.audio_base64) {
            playAudioFromBase64(response.audio_base64);
          }

          return [...prevConversation, assistantMessage];
        }

        return prevConversation;
      });

      setIsProcessing(false);
    }, 100),
    []
  );

  // Submit query function
  const submitQuery = useCallback(
    async (text: string, isFromDictation: boolean = false) => {
      console.log("[submitQuery called] Text:", text, "isFromDictation:", isFromDictation);

      if (!text.trim()) {
        console.log("[submitQuery] Returning early due to empty text.");
        return;
      }

      if (!isFromDictation && serverStatus !== "connected") {
        setConversation((prev) => [
          ...prev,
          {
            role: "system",
            content: "Cannot submit query: Server is not connected. Please wait or check connection.",
          },
        ]);
        return;
      }

      if (!isFromDictation && isProcessing) {
        console.log("[submitQuery] Returning early: query already in progress.");
        return;
      }

      const userMessage: ChatMessage = {
        role: "user",
        content: text,
        timestamp: Date.now(),
      };
      setConversation((prev) => [...prev, userMessage]);

      setQuery("");
      setIsProcessing(true);

      try {
        await invoke("submit_query", { query: text });
        console.log("submit_query invoked for:", text);
      } catch (error) {
        const errorMessage: ChatMessage = {
          role: "system",
          content: `Error invoking submit_query: ${error}`,
          timestamp: Date.now(),
        };
        setConversation((prev) => [...prev, errorMessage]);
        setIsProcessing(false);
      }
    },
    [isProcessing, serverStatus, setConversation, setQuery, setIsProcessing]
  );

  // Chat management functions
  const startNewChat = useCallback(() => {
    console.log("Starting new chat - clearing conversation");
    setConversation([]);
    setQuery("");
    setIsProcessing(false);
  }, []);

  const clearConversation = useCallback(() => {
    console.log("Clearing conversation history");
    setConversation([]);
    setIsProcessing(false);
  }, []);

  // Modal management
  const closeModal = useCallback(() => {
    setActiveModal(null);
  }, []);

  return {
    // State
    query,
    setQuery,
    conversation,
    setConversation,
    isProcessing,
    setIsProcessing,
    serverStatus,
    setServerStatus,
    currentView,
    setCurrentView,
    appVersion,
    conversationEndRef,
    currentAudio,
    setCurrentAudio,
    isDevPanelOpen,
    setIsDevPanelOpen,
    activeModal,
    setActiveModal,
    feedbackData,
    setFeedbackData,
    permissionsGranted,
    setPermissionsGranted,
    showOnboarding,
    setShowOnboarding,
    onboardingChecked,
    setOnboardingChecked,
    isCheckingUpdate,
    setIsCheckingUpdate,
    updateInfo,
    setUpdateInfo,
    isExporting,
    setIsExporting,
    isImporting,
    setIsImporting,
    copyingMessageId,
    setCopyingMessageId,
    savingMessageId,
    setSavingMessageId,

    // Functions
    handleBackendResponse,
    submitQuery,
    startNewChat,
    clearConversation,
    closeModal,
  };
};