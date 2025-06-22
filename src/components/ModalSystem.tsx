import { invoke } from "@tauri-apps/api/core";

// Types for the modal system
export type ModalType = "help" | "feedback" | "export" | "import" | "update" | null;

// Enhanced feedback form data
export interface FeedbackData {
  type: "issue" | "feature" | "general";
  title: string;
  description: string;
  email?: string;
  priority: "low" | "medium" | "high";
}

// Update check result
export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

// Chat message type for export
export type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "thinking"
    | "tool_call_request"
    | "tool_call_result";
  content: string;
  isJsx?: boolean;
  screenshot_base64?: string;
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean;
  timestamp?: number;
  isStreaming?: boolean;
  messageId?: string;
};

// Chat export format
export interface ChatExport {
  version: string;
  exported_at: string;
  conversation: ChatMessage[];
  metadata: {
    total_messages: number;
    export_type: "full" | "filtered";
  };
}

interface ModalSystemProps {
  activeModal: ModalType;
  onClose: () => void;
  feedbackData: FeedbackData;
  onFeedbackDataChange: (data: Partial<FeedbackData>) => void;
  updateInfo: UpdateInfo | null;
  conversation: ChatMessage[];
  isExporting: boolean;
  isImporting: boolean;
  keyboardShortcuts: {
    agent_mode_toggle: string;
    dictation_input: string;
    stop_current_task: string;
    open_settings: string;
  } | null;
  onUpdateConversation: (newMessages: ChatMessage[]) => void;
  onAddSystemMessage: (content: string) => void;
}

export function ModalSystem({
  activeModal,
  onClose,
  feedbackData,
  onFeedbackDataChange,
  updateInfo,
  conversation,
  isExporting,
  isImporting,
  keyboardShortcuts,
  onUpdateConversation,
  onAddSystemMessage,
}: ModalSystemProps) {
  const handleSubmitFeedback = async () => {
    if (!feedbackData.title.trim() || !feedbackData.description.trim()) {
      alert("Please fill in both title and description fields.");
      return;
    }

    try {
      console.log("📝 Submitting feedback:", feedbackData);

      // Create GitHub issue URL or mailto link for feedback
      if (feedbackData.type === "issue") {
        const title = encodeURIComponent(feedbackData.title);
        const body = encodeURIComponent(
          `**Priority:** ${feedbackData.priority}\n\n**Description:**\n${
            feedbackData.description
          }\n\n**Contact:** ${feedbackData.email || "Not provided"}`
        );
        const githubUrl = `https://github.com/lacymorrow/juno/issues/new?title=${title}&body=${body}`;

        // Open GitHub issues page
        await invoke("open_url", { url: githubUrl });
      } else {
        // For general feedback, create mailto link
        const subject = encodeURIComponent(
          `Juno AI Feedback: ${feedbackData.title}`
        );
        const body = encodeURIComponent(
          `Priority: ${feedbackData.priority}\n\nDescription:\n${feedbackData.description}`
        );
        const mailtoUrl = `mailto:feedback@juno-ai.com?subject=${subject}&body=${body}`;

        await invoke("open_url", { url: mailtoUrl });
      }

      onAddSystemMessage("✅ Feedback form opened. Thank you for your input!");

      // Reset form and close modal
      onFeedbackDataChange({
        type: "general",
        title: "",
        description: "",
        email: "",
        priority: "medium",
      });
      onClose();
    } catch (error) {
      console.error("❌ Failed to submit feedback:", error);
      onAddSystemMessage(`Failed to submit feedback: ${error}`);
    }
  };

  const handleExportChat = async () => {
    if (conversation.length === 0) {
      onAddSystemMessage("No conversation to export.");
      return;
    }

    try {
      const exportData: ChatExport = {
        version: "1.0",
        exported_at: new Date().toISOString(),
        conversation: conversation.filter((msg) => msg.role !== "system"), // Exclude system messages
        metadata: {
          total_messages: conversation.length,
          export_type: "filtered",
        },
      };

      // Use backend command to handle file save dialog and writing
      const result = (await invoke("save_chat_export", {
        data: JSON.stringify(exportData, null, 2),
      })) as { success: boolean; path?: string; error?: string };

      if (result.success && result.path) {
        onAddSystemMessage(`✅ Chat exported successfully to: ${result.path}`);
        console.log("✅ Chat exported successfully to:", result.path);
      } else {
        throw new Error(result.error || "Export failed");
      }
    } catch (error) {
      console.error("❌ Failed to export chat:", error);
      onAddSystemMessage(`Failed to export chat: ${error}`);
    } finally {
      onClose();
    }
  };

  const handleImportChat = async () => {
    try {
      // Use backend command to handle file open dialog and reading
      const result = (await invoke("load_chat_import")) as {
        success: boolean;
        data?: string;
        error?: string;
        messageCount?: number;
      };

      if (result.success && result.data) {
        const importData: ChatExport = JSON.parse(result.data);

        // Validate import format
        if (
          !importData.conversation ||
          !Array.isArray(importData.conversation)
        ) {
          throw new Error("Invalid chat export format");
        }

        // Confirm import with user
        const confirmImport = window.confirm(
          `Import ${
            result.messageCount || importData.conversation.length
          } messages? This will replace your current conversation.`
        );

        if (confirmImport) {
          // Add timestamps to imported messages if missing
          const importedMessages = importData.conversation.map((msg) => ({
            ...msg,
            timestamp: msg.timestamp || Date.now(),
          }));

          onUpdateConversation(importedMessages);
          onAddSystemMessage(
            `✅ Chat imported successfully. Loaded ${importedMessages.length} messages.`
          );
          console.log("✅ Chat imported successfully:", importData);
        }
      } else {
        if (result.error && !result.error.includes("cancelled")) {
          throw new Error(result.error);
        }
        // User cancelled - no error needed
      }
    } catch (error) {
      console.error("❌ Failed to import chat:", error);
      onAddSystemMessage(`Failed to import chat: ${error}`);
    } finally {
      onClose();
    }
  };

  const handleInstallUpdate = async () => {
    try {
      console.log("🚀 Installing update...");
      onAddSystemMessage(
        "🚀 Installing update... The application will restart automatically."
      );

      await invoke("install_update");
    } catch (error) {
      console.error("❌ Failed to install update:", error);
      onAddSystemMessage(`Failed to install update: ${error}`);
    }
  };

  if (!activeModal) return null;

  const modalContent = () => {
    switch (activeModal) {
      case "help":
        return (
          <div className="bg-background/95 backdrop-blur-xl border border-border/50 rounded-2xl p-8 max-w-3xl max-h-[85vh] overflow-y-auto shadow-2xl">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <h2 className="text-2xl font-semibold bg-gradient-to-r from-purple-700 to-indigo-700 dark:from-purple-300 dark:to-indigo-300 bg-clip-text text-transparent">
                  Help & Documentation
                </h2>
                <p className="text-sm text-muted-foreground">
                  Learn how to use Juno AI effectively
                </p>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-full hover:bg-muted/50 transition-colors duration-200 text-muted-foreground hover:text-foreground"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <div className="space-y-6 text-foreground">
              <section className="p-4 rounded-xl bg-gradient-to-r from-blue-50/50 to-indigo-50/30 dark:from-blue-950/30 dark:to-indigo-950/20 border border-blue-200/50 dark:border-blue-800/50">
                <h3 className="text-lg font-semibold mb-3 text-blue-900 dark:text-blue-100 flex items-center gap-2">
                  🎙️ Voice Controls
                </h3>
                <ul className="space-y-3 text-sm">
                  <li className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <kbd className="px-2 py-1 bg-background/80 border border-border/50 rounded text-xs font-mono">
                        {keyboardShortcuts?.agent_mode_toggle || "Option + D"}
                      </kbd>
                      <span className="font-medium">Activate Agent Mode</span>
                    </div>
                    <ul className="ml-4 space-y-1 text-xs text-muted-foreground">
                      <li><strong>Tap to Toggle:</strong> Press and release to toggle agent mode on/off</li>
                      <li><strong>Hold to Activate:</strong> Hold key to activate agent, release to stop</li>
                    </ul>
                  </li>
                  <li className="flex items-center gap-2">
                    <kbd className="px-2 py-1 bg-background/80 border border-border/50 rounded text-xs font-mono">
                      {keyboardShortcuts?.dictation_input || "Option + Space"}
                    </kbd>
                    <span className="font-medium">Toggle Dictation Mode (voice typing)</span>
                  </li>
                  <li className="flex items-center gap-2">
                    <span className="px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-200 border border-green-200 dark:border-green-800 rounded text-xs font-medium">
                      Wake Words
                    </span>
                    <span className="font-medium">Say "Hey Juno" or "Computer" (Always Listening Mode)</span>
                  </li>
                </ul>
                <p className="text-xs text-muted-foreground mt-3 p-2 bg-background/50 rounded-lg border border-border/30">
                  💡 Configure agent trigger mode in Settings → General → Agent Trigger Mode
                </p>
              </section>

              <section className="p-4 rounded-xl bg-gradient-to-r from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20 border border-green-200/50 dark:border-green-800/50">
                <h3 className="text-lg font-semibold mb-3 text-green-900 dark:text-green-100">
                  💬 Chat Features
                </h3>
                <ul className="space-y-2 text-sm">
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-green-500"></div>
                    Type your questions and press Enter
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-green-500"></div>
                    Use voice commands for hands-free interaction
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-green-500"></div>
                    Export conversations for backup or sharing
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-green-500"></div>
                    Import previous conversations to continue
                  </li>
                </ul>
              </section>

              <section className="p-4 rounded-xl bg-gradient-to-r from-purple-50/50 to-violet-50/30 dark:from-purple-950/30 dark:to-violet-950/20 border border-purple-200/50 dark:border-purple-800/50">
                <h3 className="text-lg font-semibold mb-3 text-purple-900 dark:text-purple-100">
                  🛠️ Tools & Automation
                </h3>
                <ul className="space-y-2 text-sm">
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-purple-500"></div>
                    Screen capture and analysis
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-purple-500"></div>
                    File operations and code analysis
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-purple-500"></div>
                    Web browsing automation
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-purple-500"></div>
                    System control and monitoring
                  </li>
                </ul>
              </section>

              <section className="p-4 rounded-xl bg-gradient-to-r from-orange-50/50 to-amber-50/30 dark:from-orange-950/30 dark:to-amber-950/20 border border-orange-200/50 dark:border-orange-800/50">
                <h3 className="text-lg font-semibold mb-3 text-orange-900 dark:text-orange-100">
                  ⚙️ Settings & Permissions
                </h3>
                <ul className="space-y-2 text-sm">
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-orange-500"></div>
                    Configure accessibility permissions for screen control
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-orange-500"></div>
                    Adjust voice recognition settings
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-orange-500"></div>
                    Customize keyboard shortcuts
                  </li>
                  <li className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-orange-500"></div>
                    Enable developer tools for advanced features
                  </li>
                </ul>
              </section>

              <div className="pt-4 border-t border-border/30">
                <p className="text-sm text-muted-foreground text-center">
                  For more detailed documentation, visit our GitHub repository or contact support.
                </p>
              </div>
            </div>
          </div>
        );

      case "feedback":
        return (
          <div className="bg-background/95 backdrop-blur-xl border border-border/50 rounded-2xl p-8 max-w-lg shadow-2xl">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <h2 className="text-2xl font-semibold bg-gradient-to-r from-green-700 to-emerald-700 dark:from-green-300 dark:to-emerald-300 bg-clip-text text-transparent">
                  Submit Feedback
                </h2>
                <p className="text-sm text-muted-foreground">
                  Help us improve Juno AI
                </p>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-full hover:bg-muted/50 transition-colors duration-200 text-muted-foreground hover:text-foreground"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleSubmitFeedback();
              }}
              className="space-y-5"
            >
              <div className="space-y-2">
                <label className="block text-sm font-medium text-foreground">
                  Feedback Type
                </label>
                <select
                  value={feedbackData.type}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      type: e.target.value as "issue" | "feature" | "general",
                    })
                  }
                  className="w-full p-3 border border-border/50 rounded-xl bg-background/80 backdrop-blur-sm text-foreground focus:ring-2 focus:ring-primary/20 focus:border-primary/50 transition-all duration-200"
                >
                  <option value="general">General Feedback</option>
                  <option value="issue">Bug Report</option>
                  <option value="feature">Feature Request</option>
                </select>
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-medium text-foreground">
                  Title <span className="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  value={feedbackData.title}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      title: e.target.value,
                    })
                  }
                  className="w-full p-3 border border-border/50 rounded-xl bg-background/80 backdrop-blur-sm text-foreground placeholder:text-muted-foreground focus:ring-2 focus:ring-primary/20 focus:border-primary/50 transition-all duration-200"
                  placeholder="Brief summary of your feedback"
                  required
                />
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-medium text-foreground">
                  Description <span className="text-red-500">*</span>
                </label>
                <textarea
                  value={feedbackData.description}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      description: e.target.value,
                    })
                  }
                  className="w-full p-3 border border-border/50 rounded-xl bg-background/80 backdrop-blur-sm text-foreground placeholder:text-muted-foreground focus:ring-2 focus:ring-primary/20 focus:border-primary/50 transition-all duration-200 h-28 resize-none"
                  placeholder="Detailed description of your feedback"
                  required
                />
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-medium text-foreground">
                  Priority
                </label>
                <select
                  value={feedbackData.priority}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      priority: e.target.value as "low" | "medium" | "high",
                    })
                  }
                  className="w-full p-3 border border-border/50 rounded-xl bg-background/80 backdrop-blur-sm text-foreground focus:ring-2 focus:ring-primary/20 focus:border-primary/50 transition-all duration-200"
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                </select>
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-medium text-foreground">
                  Email (Optional)
                </label>
                <input
                  type="email"
                  value={feedbackData.email || ""}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      email: e.target.value,
                    })
                  }
                  className="w-full p-3 border border-border/50 rounded-xl bg-background/80 backdrop-blur-sm text-foreground placeholder:text-muted-foreground focus:ring-2 focus:ring-primary/20 focus:border-primary/50 transition-all duration-200"
                  placeholder="your.email@example.com"
                />
              </div>

              <div className="flex gap-3 pt-4">
                <button
                  type="button"
                  onClick={onClose}
                  className="flex-1 px-6 py-3 border border-border/50 rounded-xl text-foreground hover:bg-muted/50 transition-all duration-200 font-medium"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="flex-1 px-6 py-3 bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white rounded-xl transition-all duration-200 font-medium shadow-lg hover:shadow-xl"
                >
                  Submit
                </button>
              </div>
            </form>
          </div>
        );

      case "export":
        return (
          <div className="bg-background/95 backdrop-blur-xl border border-border/50 rounded-2xl p-8 max-w-lg shadow-2xl">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <h2 className="text-2xl font-semibold bg-gradient-to-r from-blue-700 to-cyan-700 dark:from-blue-300 dark:to-cyan-300 bg-clip-text text-transparent">
                  Export Chat
                </h2>
                <p className="text-sm text-muted-foreground">
                  Save your conversation for backup or sharing
                </p>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-full hover:bg-muted/50 transition-colors duration-200 text-muted-foreground hover:text-foreground"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <div className="space-y-5">
              <p className="text-foreground">
                Export your current conversation to a JSON file for backup or sharing.
              </p>

              <div className="p-4 rounded-xl bg-gradient-to-r from-blue-50/50 to-cyan-50/30 dark:from-blue-950/30 dark:to-cyan-950/20 border border-blue-200/50 dark:border-blue-800/50 space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="font-medium text-blue-900 dark:text-blue-100">Messages to export:</span>
                  <span className="text-blue-800 dark:text-blue-200">{conversation.filter((msg) => msg.role !== "system").length}</span>
                </div>
                <div className="flex justify-between">
                  <span className="font-medium text-blue-900 dark:text-blue-100">Format:</span>
                  <span className="text-blue-800 dark:text-blue-200">JSON</span>
                </div>
                <div className="flex justify-between">
                  <span className="font-medium text-blue-900 dark:text-blue-100">Includes:</span>
                  <span className="text-blue-800 dark:text-blue-200">All user and assistant messages</span>
                </div>
              </div>

              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-6 py-3 border border-border/50 rounded-xl text-foreground hover:bg-muted/50 transition-all duration-200 font-medium"
                >
                  Cancel
                </button>
                <button
                  onClick={handleExportChat}
                  disabled={isExporting}
                  className="flex-1 px-6 py-3 bg-gradient-to-r from-blue-600 to-cyan-600 hover:from-blue-700 hover:to-cyan-700 text-white rounded-xl transition-all duration-200 font-medium shadow-lg hover:shadow-xl disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {isExporting ? "Exporting..." : "Export"}
                </button>
              </div>
            </div>
          </div>
        );

      case "import":
        return (
          <div className="bg-background/95 backdrop-blur-xl border border-border/50 rounded-2xl p-8 max-w-lg shadow-2xl">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <h2 className="text-2xl font-semibold bg-gradient-to-r from-indigo-700 to-purple-700 dark:from-indigo-300 dark:to-purple-300 bg-clip-text text-transparent">
                  Import Chat
                </h2>
                <p className="text-sm text-muted-foreground">
                  Load a previously exported conversation
                </p>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-full hover:bg-muted/50 transition-colors duration-200 text-muted-foreground hover:text-foreground"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <div className="space-y-5">
              <p className="text-foreground">
                Import a previously exported chat conversation from a JSON file.
              </p>

              <div className="p-4 rounded-xl bg-gradient-to-r from-amber-50/50 to-orange-50/30 dark:from-amber-950/30 dark:to-orange-950/20 border border-amber-200/50 dark:border-amber-800/50">
                <div className="flex items-start gap-3">
                  <div className="w-5 h-5 rounded-full bg-amber-500 flex items-center justify-center flex-shrink-0 mt-0.5">
                    <svg className="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
                      <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                    </svg>
                  </div>
                  <div className="space-y-1">
                    <p className="font-semibold text-amber-900 dark:text-amber-100 text-sm">Warning</p>
                    <p className="text-amber-800 dark:text-amber-200 text-sm">
                      This will replace your current conversation. Make sure to export it first if you want to keep it.
                    </p>
                  </div>
                </div>
              </div>

              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-6 py-3 border border-border/50 rounded-xl text-foreground hover:bg-muted/50 transition-all duration-200 font-medium"
                >
                  Cancel
                </button>
                <button
                  onClick={handleImportChat}
                  disabled={isImporting}
                  className="flex-1 px-6 py-3 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-700 hover:to-purple-700 text-white rounded-xl transition-all duration-200 font-medium shadow-lg hover:shadow-xl disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {isImporting ? "Importing..." : "Select File"}
                </button>
              </div>
            </div>
          </div>
        );

      case "update":
        return updateInfo ? (
          <div className="bg-background/95 backdrop-blur-xl border border-border/50 rounded-2xl p-8 max-w-lg shadow-2xl">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <h2 className="text-2xl font-semibold bg-gradient-to-r from-emerald-700 to-teal-700 dark:from-emerald-300 dark:to-teal-300 bg-clip-text text-transparent">
                  Update Available
                </h2>
                <p className="text-sm text-muted-foreground">
                  A new version of Juno AI is ready
                </p>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-full hover:bg-muted/50 transition-colors duration-200 text-muted-foreground hover:text-foreground"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <div className="space-y-5">
              <p className="text-foreground">
                A new version of Juno AI is available for download!
              </p>

              {updateInfo.version && (
                <div className="p-4 rounded-xl bg-gradient-to-r from-emerald-50/50 to-teal-50/30 dark:from-emerald-950/30 dark:to-teal-950/20 border border-emerald-200/50 dark:border-emerald-800/50 space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="font-medium text-emerald-900 dark:text-emerald-100">Version:</span>
                    <span className="text-emerald-800 dark:text-emerald-200 font-mono">{updateInfo.version}</span>
                  </div>
                  {updateInfo.date && (
                    <div className="flex justify-between text-sm">
                      <span className="font-medium text-emerald-900 dark:text-emerald-100">Date:</span>
                      <span className="text-emerald-800 dark:text-emerald-200">{updateInfo.date}</span>
                    </div>
                  )}
                </div>
              )}

              {updateInfo.notes && (
                <div className="p-4 rounded-xl bg-muted/30 border border-border/30 space-y-2">
                  <p className="text-sm font-semibold text-foreground">Release Notes:</p>
                  <p className="text-sm text-muted-foreground leading-relaxed">
                    {updateInfo.notes}
                  </p>
                </div>
              )}

              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-6 py-3 border border-border/50 rounded-xl text-foreground hover:bg-muted/50 transition-all duration-200 font-medium"
                >
                  Later
                </button>
                <button
                  onClick={handleInstallUpdate}
                  className="flex-1 px-6 py-3 bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-700 hover:to-teal-700 text-white rounded-xl transition-all duration-200 font-medium shadow-lg hover:shadow-xl"
                >
                  Install Update
                </button>
              </div>
            </div>
          </div>
        ) : null;

      default:
        return null;
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      {modalContent()}
    </div>
  );
}
