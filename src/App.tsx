import { AgentExecutionProgressIndicator } from "@/components/AgentExecutionProgressIndicator";
import DevToolsPanel from "@/components/DevToolsPanel";
import { ExamplePrompts } from "@/components/ExamplePrompts";
import { OnboardingFlow } from "@/components/OnboardingFlow";
import { PermissionsFlow } from "@/components/PermissionsFlow";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import { ModalManager } from "@/components/Modals";
import { MessageList, ChatInput } from "@/components/ChatInterface";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { useAppState } from "@/hooks/useAppState";
import { useEventListeners } from "@/hooks/useEventListeners";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import type { UpdateInfo } from "@/types/chat";
import {
  ArrowLeft,
  Code,
  DogIcon,
  FileText,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Server,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { FloatingBar } from "./Bar";
import ClickVisualizer from "./components/ClickVisualizer";
import Settings from "./components/Settings";
import "./styles/globals.css";
import { 
  exportChat, 
  importChat, 
  submitFeedback 
} from "@/services/chatService";

function App() {
  // Use extracted state management hook
  const {
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
    isDevPanelOpen,
    setIsDevPanelOpen,
    activeModal,
    setActiveModal,
    feedbackData,
    setFeedbackData,
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
    handleBackendResponse,
    submitQuery,
    startNewChat,
    clearConversation,
    closeModal,
  } = useAppState();

  // Set up all event listeners
  useEventListeners({
    handleBackendResponse,
    startNewChat,
    clearConversation,
    setCurrentView,
    setIsDevPanelOpen,
    setActiveModal,
    setConversation,
    setIsProcessing,
    setServerStatus,
  });

  // Onboarding completion handlers
  const handleOnboardingComplete = async () => {
    try {
      localStorage.setItem("juno-onboarding-completed", "true");
      setCurrentView("chat");

      try {
        const firstPrompt = await invoke<string>("get_first_onboarding_prompt");
        if (firstPrompt && firstPrompt.trim()) {
          await submitQuery(firstPrompt);
        }
      } catch (error) {
        console.log("No first prompt stored or error retrieving it:", error);
      }
    } catch (error) {
      console.error("Error completing onboarding:", error);
      setCurrentView("chat");
    }
  };

  const handleOnboardingSkip = () => {
    localStorage.setItem("juno-onboarding-completed", "true");
    setCurrentView("chat");
  };

  // Update check function  
  const handleUpdateCheck = async () => {
    setIsCheckingUpdate(true);
    try {
      const updateResult = await invoke<{ available: boolean; version?: string; notes?: string }>("check_for_updates");
      
      if (updateResult.available) {
        const updateInfo: UpdateInfo = {
          available: true,
          version: updateResult.version,
          notes: updateResult.notes,
        };
        setUpdateInfo(updateInfo);
        setActiveModal("update");
      } else {
        toast.success("You're running the latest version of Juno AI.");
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
      toast.error(`Failed to check for updates: ${error}`);
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  // Install update function
  const handleInstallUpdate = async () => {
    try {
      toast.info("Installing update... The application will restart automatically.");
      await invoke("install_update");
    } catch (error) {
      console.error("Failed to install update:", error);
      toast.error(`Failed to install update: ${error}`);
    }
  };

  // Chat service handlers
  const onExportChat = async () => {
    setIsExporting(true);
    try {
      await exportChat(conversation);
    } finally {
      setIsExporting(false);
      closeModal();
    }
  };
  const onImportChat = async () => {
    setIsImporting(true);
    try {
      const result = await importChat();
      // Confirm import with user
      const confirmImport = window.confirm(
        `Import ${result.messageCount} messages? This will replace your current conversation.`
      );
      
      if (confirmImport) {
        setConversation(result.messages);
      }
    } catch (error) {
      // Error is already handled by the service
    } finally {
      setIsImporting(false);
      closeModal();
    }
  };
  const onSubmitFeedback = async () => {
    try {
      await submitFeedback(feedbackData);
      // Reset form on success
      setFeedbackData({
        type: "general",
        title: "",
        description: "",
        email: "",
        priority: "medium",
      });
      closeModal();
    } catch (error) {
      // Error is already handled by the service
    }
  };

  // Main render function
  const renderMainContent = () => {
    switch (currentView) {
      case "onboarding":
        return (
          <OnboardingFlow
            onComplete={handleOnboardingComplete}
            onSkip={handleOnboardingSkip}
          />
        );

      case "permissions":
        return (
          <PermissionsFlow
            onComplete={() => setCurrentView("chat")}
            onSkip={() => setCurrentView("chat")}
          />
        );

      case "settings":
        return (
          <div className="flex flex-col h-full">
            <div className="flex items-center gap-4 p-4 border-b border-gray-200 dark:border-gray-700">
              <button
                onClick={() => setCurrentView("chat")}
                className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                title="Back to Chat"
              >
                <ArrowLeft className="w-5 h-5" />
              </button>
              <h1 className="text-xl font-bold text-gray-900 dark:text-white">
                Settings
              </h1>
            </div>
            <div className="flex-1 overflow-y-auto">
              <Settings />
            </div>
          </div>
        );

      case "devtools":
        return (
          <div className="flex flex-col h-full">
            <div className="flex items-center gap-4 p-4 border-b border-gray-200 dark:border-gray-700">
              <button
                onClick={() => setCurrentView("chat")}
                className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                title="Back to Chat"
              >
                <ArrowLeft className="w-5 h-5" />
              </button>
              <h1 className="text-xl font-bold text-gray-900 dark:text-white">
                Developer Tools
              </h1>
            </div>
            <div className="flex-1 overflow-y-auto">
              <DevToolsPanel />
            </div>
          </div>
        );

      case "chat":
      default:
        return (
          <ResizablePanelGroup direction="horizontal" className="flex-1">
            {/* Main Chat Panel */}
            <ResizablePanel defaultSize={isDevPanelOpen ? 75 : 100}>
              <div className="flex flex-col h-full bg-white dark:bg-gray-900">
                {/* Header */}
                <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
                  <div className="flex items-center gap-3">
                    <DogIcon className="w-8 h-8 text-blue-500" />
                    <div>
                      <h1 className="text-xl font-bold text-gray-900 dark:text-white">
                        Juno AI Assistant
                      </h1>
                      <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
                        <span>{appVersion}</span>
                        <div className="flex items-center gap-1">
                          <Server className="w-3 h-3" />
                          <span
                            className={cn(
                              "text-xs",
                              serverStatus === "connected"
                                ? "text-green-600 dark:text-green-400"
                                : serverStatus === "checking"
                                ? "text-yellow-600 dark:text-yellow-400"
                                : "text-red-600 dark:text-red-400"
                            )}
                          >
                            {serverStatus}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    <VoiceStatusIndicator />
                    <AgentExecutionProgressIndicator />
                    
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setActiveModal("help")}
                      title="Help & Documentation"
                    >
                      <FileText className="w-4 h-4" />
                    </Button>

                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setCurrentView("settings")}
                      title="Settings"
                    >
                      <Code className="w-4 h-4" />
                    </Button>

                    <Button
                      variant="outline"
                      size="sm"
                      onClick={startNewChat}
                      title="New Chat"
                    >
                      <Plus className="w-4 h-4" />
                    </Button>

                    <Button
                      variant="outline"
                      size="sm"
                      onClick={clearConversation}
                      title="Clear History"
                    >
                      <Trash2 className="w-4 h-4" />
                    </Button>

                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setIsDevPanelOpen(!isDevPanelOpen)}
                      title={isDevPanelOpen ? "Hide Dev Panel" : "Show Dev Panel"}
                    >
                      {isDevPanelOpen ? (
                        <PanelLeftClose className="w-4 h-4" />
                      ) : (
                        <PanelLeftOpen className="w-4 h-4" />
                      )}
                    </Button>
                  </div>
                </div>

                {/* Chat Area */}
                {conversation.length === 0 ? (
                  <div className="flex-1 flex items-center justify-center p-8">
                    <ExamplePrompts onPromptSelect={submitQuery} />
                  </div>
                ) : (
                  <MessageList
                    conversation={conversation}
                    copyingMessageId={copyingMessageId}
                    setCopyingMessageId={setCopyingMessageId}
                    savingMessageId={savingMessageId}
                    setSavingMessageId={setSavingMessageId}
                    conversationEndRef={conversationEndRef}
                  />
                )}

                {/* Chat Input */}
                <ChatInput
                  query={query}
                  setQuery={setQuery}
                  isProcessing={isProcessing}
                  serverStatus={serverStatus}
                  onSubmit={submitQuery}
                />
              </div>
            </ResizablePanel>

            {/* Dev Panel */}
            {isDevPanelOpen && (
              <>
                <ResizableHandle />
                <ResizablePanel defaultSize={25} minSize={20} maxSize={50}>
                  <div className="h-full border-l border-gray-200 dark:border-gray-700">
                    <DevToolsPanel />
                  </div>
                </ResizablePanel>
              </>
            )}
          </ResizablePanelGroup>
        );
    }
  };

  return (
    <div className="flex flex-col h-screen bg-gray-50 dark:bg-gray-900">
      {/* Main Content */}
      {renderMainContent()}

      {/* Floating Bar */}
      <FloatingBar />

      {/* Click Visualizer */}
      <ClickVisualizer />

      {/* Modals */}
      {activeModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <ModalManager
            activeModal={activeModal}
            onClose={closeModal}
            feedbackData={feedbackData}
            setFeedbackData={setFeedbackData}
            onSubmitFeedback={onSubmitFeedback}
            conversation={conversation}
            onExportChat={onExportChat}
            isExporting={isExporting}
            onImportChat={onImportChat}
            isImporting={isImporting}
            updateInfo={updateInfo}
            onInstallUpdate={handleInstallUpdate}
          />
        </div>
      )}
    </div>
  );
}

export default App;
