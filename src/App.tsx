import './styles/App.css'

import React, { useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { toast } from "sonner";

import { AppHeader } from "@/components/AppHeader";
import DevToolsPanel from "@/components/DevToolsPanel";
import { ModalSystem } from "@/components/ModalSystem";
import { PermissionsFlow } from "@/components/PermissionsFlow";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";

import { ChatContainer, ChatInput } from "@/components/chat";
import ClickVisualizer from "@/components/ClickVisualizer";
import CommandOverlay from "@/components/CommandOverlay";
import KeyPressOverlay from "@/components/KeyPressOverlay";
import ToolApprovalModal from "@/components/ToolApprovalModal";

// Custom hooks
import { useAppState } from "@/hooks/useAppState";
import { useConversation } from "@/hooks/useConversation";
import { useAudioPlayback } from "@/hooks/useAudioPlayback";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import { useMenuEvents } from "@/hooks/useMenuEvents";
import { useChatScrolling } from "@/hooks/useChatScrolling";
import { useSound, useVoiceSounds } from "@/hooks/useSound";

function App() {
  // Initialize custom hooks
  const appState = useAppState();
  const conversation = useConversation();
  const audioPlayback = useAudioPlayback();
  const { playError } = useSound();

  // Use voice sounds hook
  useVoiceSounds();

  // Scrolling management
  const scrolling = useChatScrolling({
    conversation: conversation.conversation,
    userHasScrolledUp: appState.userHasScrolledUp,
    lastScrollTime: appState.lastScrollTime,
    setUserHasScrolledUp: appState.setUserHasScrolledUp,
    setLastScrollTime: appState.setLastScrollTime,
  });

  // Enhanced submit handler
  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!appState.canSubmit || !conversation.query.trim()) return;

      const trimmedQuery = conversation.query.trim();
      console.log("🚀 Submitting query:", trimmedQuery);

      appState.setIsProcessing(true);
      conversation.addUserMessage(trimmedQuery);
      conversation.setQuery("");

      try {
        await invoke("submit_query", { query: trimmedQuery });
        console.log("✅ Query submitted successfully");
      } catch (error) {
        console.error("❌ Failed to submit query:", error);
        conversation.addSystemMessage(`Failed to submit query: ${error}`);
        appState.setIsProcessing(false);
        playError();
      }
    },
    [
      appState.canSubmit,
      conversation.query,
      appState.setIsProcessing,
      conversation.addUserMessage,
      conversation.setQuery,
      conversation.addSystemMessage,
      playError,
    ]
  );

  // Enhanced stop handler
  const handleStop = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      console.log("🛑 Stop requested by user");

      try {
        await audioPlayback.stopAllAudio();
        await invoke("stop_all_operations");
        console.log("✅ All operations stopped successfully");
        conversation.addSystemMessage("🛑 All operations stopped by user");
      } catch (error) {
        console.error("❌ Error stopping operations:", error);
        conversation.addSystemMessage(`❌ Error stopping operations: ${error}`);
      }
    },
    [audioPlayback.stopAllAudio, conversation.addSystemMessage]
  );

  // Update check handler
  const handleUpdateCheck = useCallback(async () => {
    if (appState.isCheckingUpdate) return;

    appState.setIsCheckingUpdate(true);
    try {
      console.log("🔍 Checking for updates...");
      const updateAvailable: boolean = await invoke("check_for_updates");

      if (updateAvailable) {
        const latestVersion: string = await invoke("get_latest_version");
        appState.setUpdateInfo({
          available: true,
          version: latestVersion,
        });
        appState.setActiveModal("update");
        console.log(`✅ Update available: v${latestVersion}`);
      } else {
        toast.success("✅ You're running the latest version!");
        console.log("✅ No updates available - you're on the latest version");
      }
    } catch (error) {
      console.error("❌ Error checking for updates:", error);
      toast.error(`❌ Failed to check for updates: ${error}`);
    } finally {
      appState.setIsCheckingUpdate(false);
    }
  }, [
    appState.isCheckingUpdate,
    appState.setIsCheckingUpdate,
    appState.setUpdateInfo,
    appState.setActiveModal,
  ]);

  // Backend events integration
  useBackendEvents({
    addSystemMessage: conversation.addSystemMessage,
    addAssistantMessage: conversation.addAssistantMessage,
    setConversationWithPruning: conversation.setConversationWithPruning,
    playAudioFromBase64: audioPlayback.playAudioFromBase64,
    stopCurrentAudio: audioPlayback.stopCurrentAudio,
    setIsProcessing: appState.setIsProcessing,
    setServerStatus: appState.setServerStatus,
    setUserHasScrolledUp: appState.setUserHasScrolledUp,
    throttledAutoScroll: scrolling.throttledAutoScroll,
  });

  // Menu events integration
  useMenuEvents({
    setCurrentView: appState.setCurrentView,
    setIsDevPanelOpen: appState.setIsDevPanelOpen,
    setActiveModal: appState.setActiveModal,
    setFeedbackData: appState.setFeedbackData,
    startNewChat: conversation.startNewChat,
    clearConversation: conversation.clearConversation,
    addSystemMessage: conversation.addSystemMessage,
    handleUpdateCheck,
  });

  // Load app metadata on startup
  useEffect(() => {
    const initializeApp = async () => {
      try {
        // Load app version
        const version = await getVersion();
        appState.setAppVersion(version);

        // Load keyboard shortcuts
        const shortcuts = (await invoke("get_keyboard_shortcuts")) as {
          agent_mode_toggle: string;
          dictation_input: string;
          stop_current_task: string;
          open_settings: string;
        };
        appState.setKeyboardShortcuts(shortcuts);

        console.log(`🚀 Juno AI v${version} initialized`);
      } catch (error) {
        console.error("❌ Failed to initialize app:", error);
      }
    };

    initializeApp();
  }, [appState.setAppVersion, appState.setKeyboardShortcuts]);

  // Enhanced keyboard event handling
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (appState.activeModal) return;

      // Option+D for Agent Mode trigger
      if (event.altKey && event.key.toLowerCase() === "d") {
        event.preventDefault();
        console.log("🎤 Agent mode triggered via Option+D");
        invoke("trigger_agent_mode").catch(console.error);
        return;
      }

      // Option+S for Settings
      if (event.altKey && event.key.toLowerCase() === "s") {
        event.preventDefault();
        console.log("⚙️ Opening settings via Option+S");
        invoke("open_settings_window").catch(console.error);
        return;
      }

      // Escape key handling
      if (event.key === "Escape") {
        event.preventDefault();
        if (appState.isProcessing) {
          console.log("🛑 Escape pressed - stopping operations");
          handleStop({ preventDefault: () => {} } as React.FormEvent);
        }
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [appState.activeModal, appState.isProcessing, handleStop]);

  // Example prompt selection handler
  const handleExamplePromptSelect = useCallback(
    (prompt: string) => {
      conversation.setQuery(prompt);
      scrolling.autoScrollToBottom(true);
    },
    [conversation.setQuery, scrolling.autoScrollToBottom]
  );

  // Copy response handler
  const handleCopyResponse = useCallback(
    (content: string, messageIndex: number) => {
      conversation.handleCopyResponse(
        content,
        messageIndex,
        appState.setCopyingMessageId
      );
    },
    [conversation.handleCopyResponse, appState.setCopyingMessageId]
  );

  // Save response handler
  const handleSaveResponse = useCallback(
    (content: string, format: "html" | "markdown", messageIndex: number) => {
      conversation.handleSaveResponse(
        content,
        format,
        messageIndex,
        appState.setSavingMessageId
      );
    },
    [conversation.handleSaveResponse, appState.setSavingMessageId]
  );

  // Render main UI
  return (
    <div className="flex flex-col h-screen bg-gradient-to-br from-background via-background/95 to-muted/20 overflow-hidden">
      {/* Header - Enhanced with glass effect */}
      <AppHeader
        currentView={appState.currentView}
        onViewChange={appState.setCurrentView}
        onToggleDevPanel={appState.toggleDevPanel}
        serverStatus={appState.serverStatus}
        isProcessing={appState.isProcessing}
        isDevPanelOpen={appState.isDevPanelOpen}
      />

      {/* Main Content - Enhanced with better spacing and glass effects */}
      <div className="flex-1 min-h-0 relative">
        <ResizablePanelGroup direction="horizontal" className="h-full">
          {/* Primary Content Panel - Enhanced styling */}
          <ResizablePanel defaultSize={appState.isDevPanelOpen ? 70 : 100}>
            <div className="flex flex-col h-full bg-background/50 backdrop-blur-sm">
              {/* Content Area - Enhanced with better padding and glass effects */}
              <div className="flex-1 min-h-0 p-6">
                {appState.currentView === "chat" && (
                  <div className="flex flex-col h-full space-y-3 max-w-4xl mx-auto">
                    {/* Chat Container with enhanced styling */}
                    <div className="flex-1 min-h-0 rounded-xl bg-background/80 backdrop-blur-sm border border-border/30 shadow-sm overflow-hidden">
                      <ChatContainer
                        conversation={conversation.conversation}
                        copyingMessageId={appState.copyingMessageId}
                        savingMessageId={appState.savingMessageId}
                        userHasScrolledUp={appState.userHasScrolledUp}
                        lastScrollTime={appState.lastScrollTime}
                        setUserHasScrolledUp={appState.setUserHasScrolledUp}
                        setLastScrollTime={appState.setLastScrollTime}
                        onCopyResponse={handleCopyResponse}
                        onSaveResponse={handleSaveResponse}
                        onExamplePromptSelect={handleExamplePromptSelect}
                      />
                    </div>

                    {/* Chat Input with enhanced styling */}
                    <div className="rounded-xl bg-background/90 backdrop-blur-sm border border-border/30 shadow-sm overflow-hidden">
                      <ChatInput
                        query={conversation.query}
                        isProcessing={appState.isProcessing}
                        canSubmit={appState.canSubmit}
                        onQueryChange={conversation.setQuery}
                        onSubmit={handleSubmit}
                        onStop={handleStop}
                        onNewChat={conversation.startNewChat}
                        onClearConversation={conversation.clearConversation}
                      />
                    </div>
                  </div>
                )}

                {appState.currentView === "permissions" && (
                  <div className="h-full flex items-center justify-center">
                    <div className="w-full max-w-2xl">
                      <PermissionsFlow />
                    </div>
                  </div>
                )}
              </div>
            </div>
          </ResizablePanel>

          {/* Dev Tools Panel - Enhanced styling */}
          {appState.isDevPanelOpen && (
            <>
              <ResizableHandle className="w-1 bg-border/30 hover:bg-border/50 transition-colors duration-200" />
              <ResizablePanel defaultSize={30} minSize={25} maxSize={50}>
                <div className="h-full bg-background/30 backdrop-blur-sm border-l border-border/30">
                  <DevToolsPanel />
                </div>
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>

      {/* Overlays - Enhanced with better z-index management */}
      <div className="relative z-50">
        <ClickVisualizer />
        <CommandOverlay />
        <KeyPressOverlay />
        <ToolApprovalModal />
      </div>

      {/* Modal System - Enhanced positioning */}
      <div className="relative z-[100]">
        <ModalSystem
          activeModal={appState.activeModal}
          onClose={() => appState.setActiveModal(null)}
          feedbackData={appState.feedbackData}
          onFeedbackDataChange={appState.handleFeedbackDataChange}
          updateInfo={appState.updateInfo}
          conversation={conversation.conversation}
          isExporting={false}
          isImporting={false}
          keyboardShortcuts={appState.keyboardShortcuts}
          onUpdateConversation={conversation.updateConversation}
          onAddSystemMessage={conversation.addSystemMessage}
        />
      </div>
    </div>
  );
}

export default App;
