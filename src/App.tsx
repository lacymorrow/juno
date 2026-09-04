import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { toast } from "sonner";
import { COMMANDS } from "@/lib/constants.generated";

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
import { AgentSessionSwitcher } from "@/components/AgentSessionSwitcher";
import ClickVisualizer from "@/components/ClickVisualizer";
import CommandOverlay from "@/components/CommandOverlay";
import KeyPressOverlay from "@/components/KeyPressOverlay";
// Inline tool approval handled in ChatMessageV2

// Custom hooks
import { useAppState } from "@/hooks/useAppState";
import { useConversation } from "@/hooks/useConversation";
import { useOnboardingState } from "@/hooks/useOnboardingState";
import { useAudioPlayback } from "@/hooks/useAudioPlayback";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import { useMenuEvents } from "@/hooks/useMenuEvents";
import { useSound, useVoiceSounds } from "@/hooks/useSound";
import { useShortcutEvents } from "@/hooks/useShortcutEvents";
import { useDictationStateEvents } from "@/hooks/useDictationStateEvents";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useUpdater } from "@/hooks/useUpdater";

function App() {
  // Initialize custom hooks
  const appState = useAppState();
  const conversation = useConversation();
  const { isOnboardingActive } = useOnboardingState();
  const audioPlayback = useAudioPlayback();
  const { playError } = useSound();
  const agentSessions = useAgentSessions();

  const { checkForUpdates, installUpdate } = useUpdater();

  // Timer tracking for cleanup
  const pendingTimers = useRef<ReturnType<typeof setTimeout>[]>([]);
  useEffect(() => {
    return () => {
      for (const timer of pendingTimers.current) {
        clearTimeout(timer);
      }
    };
  }, []);

  // Use voice sounds hook
  useVoiceSounds();

  // The single submission path for every query this window originates
  // (typed input and example prompts). The backend owns everything after
  // the invoke: it emits `user-message-submitted` (which appends the message
  // and switches this window to processing), drives the floating bar, and
  // runs the agent. Voice, the bar, and agent-rendered components enter the
  // same pipeline, so every source behaves identically.
  const handleSubmit = useCallback(
    async (text: string) => {
      const query = text.trim();
      if (!appState.canSubmit || !query) return;

      console.log("🚀 Submitting query:", query);
      conversation.setQuery("");

      try {
        await invoke(COMMANDS.AGENT_DISPATCH_QUERY, { query });
      } catch (error) {
        console.error("❌ Failed to submit query:", error);
        conversation.addSystemMessage(`Failed to submit query: ${error}`);
        playError();
      }
    },
    [
      appState.canSubmit,
      conversation.setQuery,
      conversation.addSystemMessage,
      playError,
    ],
  );

  // Enhanced stop handler — called directly by PromptInputSubmit onStop
  const handleStop = useCallback(async () => {
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
  }, [audioPlayback.stopAllAudio, conversation.addSystemMessage]);

  // Update check handler
  const handleUpdateCheck = useCallback(async () => {
    if (appState.isCheckingUpdate) return;

    appState.setIsCheckingUpdate(true);
    try {
      const { available, info } = await checkForUpdates();
      if (available && info) {
        appState.setUpdateInfo(info);
        appState.setActiveModal("update");
      } else {
        toast.success("✅ You're running the latest version!");
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
    checkForUpdates,
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
  });

  // Menu events integration
  useMenuEvents({
    setCurrentView: appState.setCurrentView,
    setIsDevPanelOpen: appState.setIsDevPanelOpen,
    setActiveModal: appState.setActiveModal,
    setFeedbackData: appState.setFeedbackData,
    startNewChat: conversation.startNewChat,
    addSystemMessage: conversation.addSystemMessage,
    handleUpdateCheck,
  });

  // Shortcut events integration
  useShortcutEvents({
    onAgentModeShortcut: useCallback(
      (payload: any) => {
        console.log("Agent mode shortcut event received:", payload);
        if (payload.state === "pressed" && !payload.test_mode) {
          appState.setIsAgentModeActive(true);
          toast.info("Agent mode activated");
        } else if (payload.state === "released") {
          appState.setIsAgentModeActive(false);
        }
      },
      [appState],
    ),
    onDictationInputShortcut: useCallback((payload: any) => {
      console.log("Dictation input shortcut event received:", payload);
      if (payload.state === "pressed" && !payload.test_mode) {
        // Don't toggle local state - the backend will handle it and emit events
        // The frontend will update based on those events
      } else if (payload.state === "released") {
        // For hold mode - handled by backend
      }
    }, []),
  });

  // Dictation state events integration
  useDictationStateEvents({
    onStateChanged: useCallback(
      (event: any) => {
        console.log("Dictation state changed:", event);
        appState.setDictationState(event.new_state);

        // Update UI based on dictation state changes
        if (event.new_state === "active") {
          appState.setIsDictationActive(true);
        } else if (event.new_state === "idle") {
          appState.setIsDictationActive(false);
        }
      },
      [appState],
    ),
    onForceReset: useCallback(
      (reason: any) => {
        console.log("Dictation force reset:", reason);
        appState.setDictationState("idle");
        appState.setIsDictationActive(false);
        toast.error(`Dictation reset: ${reason}`);
      },
      [appState],
    ),
    onInputChanged: useCallback((state: any) => {
      console.log("Dictation input state changed:", state);
      // Update input state tracking - could be used for visual feedback
    }, []),
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
          agent_mode: string;
          dictation_input: string;
          stop_current_task: string;
          open_settings: string;
        };
        appState.setKeyboardShortcuts(shortcuts);

        // Initialize cloud connector event listeners (but don't start connection yet)
        // The connection will be started when user clicks "Start Connector" in settings
        console.log("🌐 Cloud connector event listeners initialized");

        console.log(`🚀 Juno AI v${version} initialized`);
      } catch (error) {
        console.error("❌ Failed to initialize app:", error);
      }
    };

    initializeApp();
  }, [appState.setAppVersion, appState.setKeyboardShortcuts]);

  // Listen for dictation-active events from backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    let currentDictationState = false;
    let lastToastTime = 0;
    const MIN_TOAST_INTERVAL = 1000;

    const setupListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const fn = await listen("dictation-active", (event) => {
          if (!mounted) return;
          const isActive = event.payload as boolean;
          console.log("Dictation active event from backend:", isActive);

          appState.setIsDictationActive(isActive);

          const now = Date.now();
          if (
            currentDictationState !== isActive &&
            now - lastToastTime > MIN_TOAST_INTERVAL
          ) {
            currentDictationState = isActive;
            lastToastTime = now;

            const toastId = `dictation-${isActive ? "on" : "off"}`;
            toast.dismiss("dictation-on");
            toast.dismiss("dictation-off");
            toast.info(
              isActive
                ? "Dictation mode activated"
                : "Dictation mode deactivated",
              { id: toastId, duration: 2000 },
            );
          }

          if (!isActive) {
            appState.setDictationState("idle");
          }
        });
        if (mounted) {
          unlisten = fn;
        } else {
          fn();
        }
      } catch (error) {
        console.error("Failed to setup dictation listener:", error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      unlisten?.();
      toast.dismiss("dictation-on");
      toast.dismiss("dictation-off");
    };
  }, [appState.setIsDictationActive, appState.setDictationState]);

  // Note: Keyboard shortcuts are handled entirely by the Rust backend via Tauri's global shortcut system
  // Frontend no longer needs to handle keyboard events for business logic - keeps UI truly "dumb"
  //
  // The backend escape key system works correctly:
  // 1. Escape pressed → Backend stop coordinator stops all operations
  // 2. Backend emits events → Frontend receives and stops audio/UI
  // 3. No frontend state checks needed - escape universally stops everything
  //
  // This design prevents the original bug where frontend state checks could fail,
  // while providing reliable universal cancellation behavior.

  // Example prompt selection — shows the prompt in the input for a beat,
  // then submits it through the same path as typed input.
  const handleExamplePromptSelect = useCallback(
    (prompt: string) => {
      const trimmedPrompt = prompt.trim();
      if (!appState.canSubmit || !trimmedPrompt) return;

      console.log("🚀 Auto-submitting example prompt:", trimmedPrompt);
      conversation.setQuery(trimmedPrompt);

      pendingTimers.current.push(
        setTimeout(() => {
          void handleSubmit(trimmedPrompt);
        }, 100),
      );
    },
    [appState.canSubmit, conversation.setQuery, handleSubmit],
  );

  // Copy response handler
  const handleCopyResponse = useCallback(
    (content: string, messageIndex: number) => {
      conversation.handleCopyResponse(
        content,
        messageIndex,
        appState.setCopyingMessageId,
      );
    },
    [conversation.handleCopyResponse, appState.setCopyingMessageId],
  );

  // Save response handler
  const handleSaveResponse = useCallback(
    (content: string, format: "html" | "markdown", messageIndex: number) => {
      conversation.handleSaveResponse(
        content,
        format,
        messageIndex,
        appState.setSavingMessageId,
      );
    },
    [conversation.handleSaveResponse, appState.setSavingMessageId],
  );

  // Inline tool approval handler — updates message approval_state in conversation
  const handleApprovalUpdate = useCallback(
    (toolId: string, state: "approved" | "denied") => {
      conversation.setConversationWithPruning((prev) =>
        prev.map((msg) =>
          msg.tool_id === toolId ? { ...msg, approval_state: state } : msg,
        ),
      );
    },
    [conversation.setConversationWithPruning],
  );

  const handleContinuationUpdate = useCallback(
    (requestId: string, state: "stopped" | "continued") => {
      conversation.setConversationWithPruning((prev) =>
        prev.map((msg) =>
          msg.continuation_request_id === requestId
            ? { ...msg, continuation_state: state }
            : msg,
        ),
      );
    },
    [conversation.setConversationWithPruning],
  );

  // Render main UI
  return (
    <div className="flex flex-col h-screen bg-background overflow-hidden antialiased">
      {/* Header - Fixed to include required props for model/agent selection */}
      <AppHeader
        currentView={appState.currentView}
        onViewChange={appState.setCurrentView}
        onToggleDevPanel={appState.toggleDevPanel}
        serverStatus={appState.serverStatus}
        isProcessing={appState.isProcessing}
        isDevPanelOpen={appState.isDevPanelOpen}
        onNewChat={conversation.startNewChat}
      />

      {/* Main Content */}
      <div className="flex-1 min-h-0">
        <ResizablePanelGroup direction="horizontal">
          {/* Primary Content Panel */}
          <ResizablePanel defaultSize={appState.isDevPanelOpen ? 70 : 100}>
            <div className="flex flex-col h-full">
              {/* Content Area */}
              <div className="flex-1 min-h-0 px-0 pt-0 pb-4">
                {appState.currentView === "chat" && (
                  <div className="flex flex-col h-full">
                    <ChatContainer
                      conversation={conversation.conversation}
                      copyingMessageId={appState.copyingMessageId}
                      savingMessageId={appState.savingMessageId}
                      onCopyResponse={handleCopyResponse}
                      onSaveResponse={handleSaveResponse}
                      onExamplePromptSelect={handleExamplePromptSelect}
                      onApprovalUpdate={handleApprovalUpdate}
                      onContinuationUpdate={handleContinuationUpdate}
                    />

                    <AgentSessionSwitcher
                      sessions={agentSessions.sessions}
                      onFocus={agentSessions.focusSession}
                      onCancel={agentSessions.cancelSession}
                    />

                    <div className="px-3">
                      <ChatInput
                        query={conversation.query}
                        isProcessing={appState.isProcessing}
                        canSubmit={appState.canSubmit && !isOnboardingActive}
                        onQueryChange={conversation.setQuery}
                        onSubmit={handleSubmit}
                        onStop={handleStop}
                        placeholder={isOnboardingActive ? "Use the buttons above to proceed through setup…" : undefined}
                      />
                    </div>
                  </div>
                )}

                {appState.currentView === "permissions" && <PermissionsFlow />}
              </div>
            </div>
          </ResizablePanel>

          {/* Dev Tools Panel */}
          {appState.isDevPanelOpen && (
            <>
              <ResizableHandle />
              <ResizablePanel defaultSize={42} minSize={25} maxSize={70}>
                <DevToolsPanel />
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>

      {/* Overlays */}
      <ClickVisualizer />
      <CommandOverlay />
      <KeyPressOverlay />
      {/* Inline tool approval is now handled in ChatMessageV2 */}

      {/* Modal System - Fixed to match expected props */}
      <ModalSystem
        activeModal={appState.activeModal}
        onClose={() => appState.setActiveModal(null)}
        feedbackData={appState.feedbackData}
        onFeedbackDataChange={appState.handleFeedbackDataChange}
        updateInfo={appState.updateInfo}
        onInstallUpdate={installUpdate}
        conversation={conversation.conversation}
        isExporting={false}
        isImporting={false}
        keyboardShortcuts={appState.keyboardShortcuts}
        onUpdateConversation={conversation.updateConversation}
        onAddSystemMessage={conversation.addSystemMessage}
      />
    </div>
  );
}

export default App;
