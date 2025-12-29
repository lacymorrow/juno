import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { KEYBOARD_SHORTCUTS } from "@/lib/constants.generated";

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
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-2xl max-h-[80vh] overflow-y-auto">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-bold text-gray-900 dark:text-white">
                Help & Documentation
              </h2>
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg
                  className="w-6 h-6"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
            <div className="space-y-4 text-gray-700 dark:text-gray-300">
              <section>
                <h3 className="text-lg font-semibold mb-2">
                  🎙️ Voice Controls
                </h3>
                <ul className="list-disc list-inside space-y-1">
                  <li>
                    <strong>
                      {keyboardShortcuts?.agent_mode_toggle || KEYBOARD_SHORTCUTS.AGENT_MODE_TOGGLE}:
                    </strong>{" "}
                    Activate Agent Mode
                    <ul className="list-disc list-inside ml-4 mt-1 space-y-1 text-sm">
                      <li>
                        <strong>Tap to Toggle:</strong> Press and release to
                        toggle agent mode on/off
                      </li>
                      <li>
                        <strong>Hold to Activate:</strong> Hold key to
                        activate agent, release to stop
                      </li>
                    </ul>
                  </li>
                  <li>
                    <strong>
                      {keyboardShortcuts?.dictation_input || KEYBOARD_SHORTCUTS.DICTATION_INPUT}
                      :
                    </strong>{" "}
                    Toggle Dictation Mode (voice typing)
                  </li>
                  <li>
                    <strong>Wake Words:</strong> Say "Hey Juno" or "Computer"
                    (Always Listening Mode)
                  </li>
                </ul>
                <p className="text-xs text-gray-500 mt-2">
                  Configure agent trigger mode in Settings → General → Agent
                  Trigger Mode
                </p>
              </section>
              <section>
                <h3 className="text-lg font-semibold mb-2">
                  💬 Chat Features
                </h3>
                <ul className="list-disc list-inside space-y-1">
                  <li>Type your questions and press Enter</li>
                  <li>Use voice commands for hands-free interaction</li>
                  <li>Export conversations for backup or sharing</li>
                  <li>Import previous conversations to continue</li>
                </ul>
              </section>
              <section>
                <h3 className="text-lg font-semibold mb-2">
                  🛠️ Tools & Automation
                </h3>
                <ul className="list-disc list-inside space-y-1">
                  <li>Screen capture and analysis</li>
                  <li>File operations and code analysis</li>
                  <li>Web browsing automation</li>
                  <li>System control and monitoring</li>
                </ul>
              </section>
              <section>
                <h3 className="text-lg font-semibold mb-2">
                  ⚙️ Settings & Permissions
                </h3>
                <ul className="list-disc list-inside space-y-1">
                  <li>
                    Configure accessibility permissions for screen control
                  </li>
                  <li>Adjust voice recognition settings</li>
                  <li>Customize keyboard shortcuts</li>
                  <li>Enable developer tools for advanced features</li>
                </ul>
              </section>
              <div className="pt-4 border-t border-gray-200 dark:border-gray-600">
                <p className="text-sm text-gray-600 dark:text-gray-400">
                  For more detailed documentation, visit our GitHub repository
                  or contact support.
                </p>
              </div>
            </div>
          </div>
        );

      case "feedback":
        return (
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-bold text-gray-900 dark:text-white">
                Submit Feedback
              </h2>
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg
                  className="w-6 h-6"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleSubmitFeedback();
              }}
              className="space-y-4"
            >
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Feedback Type
                </label>
                <select
                  value={feedbackData.type}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      type: e.target.value as "issue" | "feature" | "general",
                    })
                  }
                  className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                >
                  <option value="general">General Feedback</option>
                  <option value="issue">Bug Report</option>
                  <option value="feature">Feature Request</option>
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Title *
                </label>
                <input
                  type="text"
                  value={feedbackData.title}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      title: e.target.value,
                    })
                  }
                  className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                  placeholder="Brief summary of your feedback"
                  required
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Description *
                </label>
                <textarea
                  value={feedbackData.description}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      description: e.target.value,
                    })
                  }
                  className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white h-24"
                  placeholder="Detailed description of your feedback"
                  required
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Priority
                </label>
                <select
                  value={feedbackData.priority}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      priority: e.target.value as "low" | "medium" | "high",
                    })
                  }
                  className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Email (Optional)
                </label>
                <input
                  type="email"
                  value={feedbackData.email}
                  onChange={(e) =>
                    onFeedbackDataChange({
                      email: e.target.value,
                    })
                  }
                  className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                  placeholder="your.email@example.com"
                />
              </div>
              <div className="flex gap-3 pt-2">
                <button
                  type="button"
                  onClick={onClose}
                  className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600"
                >
                  Submit
                </button>
              </div>
            </form>
          </div>
        );

      case "export":
        return (
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-bold text-gray-900 dark:text-white">
                Export Chat
              </h2>
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg
                  className="w-6 h-6"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
            <div className="space-y-4">
              <p className="text-gray-700 dark:text-gray-300">
                Export your current conversation to a JSON file for backup or
                sharing.
              </p>
              <div className="bg-gray-50 dark:bg-gray-700 p-3 rounded text-sm">
                <strong>Messages to export:</strong>{" "}
                {conversation.filter((msg) => msg.role !== "system").length}
                <br />
                <strong>Format:</strong> JSON
                <br />
                <strong>Includes:</strong> All user and assistant messages
              </div>
              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                >
                  Cancel
                </button>
                <button
                  onClick={handleExportChat}
                  disabled={isExporting}
                  className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600 disabled:opacity-50"
                >
                  {isExporting ? "Exporting..." : "Export"}
                </button>
              </div>
            </div>
          </div>
        );

      case "import":
        return (
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-bold text-gray-900 dark:text-white">
                Import Chat
              </h2>
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg
                  className="w-6 h-6"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
            <div className="space-y-4">
              <p className="text-gray-700 dark:text-gray-300">
                Import a previously exported chat conversation from a JSON
                file.
              </p>
              <div className="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 p-3 rounded text-sm text-yellow-800 dark:text-yellow-200">
                <strong>Warning:</strong> This will replace your current
                conversation. Make sure to export it first if you want to keep
                it.
              </div>
              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                >
                  Cancel
                </button>
                <button
                  onClick={handleImportChat}
                  disabled={isImporting}
                  className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50"
                >
                  {isImporting ? "Importing..." : "Select File"}
                </button>
              </div>
            </div>
          </div>
        );

      case "update":
        return updateInfo ? (
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-bold text-gray-900 dark:text-white">
                Update Available
              </h2>
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg
                  className="w-6 h-6"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
            <div className="space-y-4">
              <div className="space-y-2">
                <p className="text-gray-700 dark:text-gray-300">
                  A new version of Juno AI is available!
                </p>
                {updateInfo.version && (
                  <div className="bg-blue-50 dark:bg-blue-900/20 p-3 rounded">
                    <p className="text-sm">
                      <strong>Version:</strong> {updateInfo.version}
                    </p>
                    {updateInfo.date && (
                      <p className="text-sm">
                        <strong>Date:</strong> {updateInfo.date}
                      </p>
                    )}
                  </div>
                )}
                {updateInfo.notes && (
                  <div className="bg-gray-50 dark:bg-gray-700 p-3 rounded">
                    <p className="text-sm font-medium mb-1">Release Notes:</p>
                    <p className="text-sm text-gray-600 dark:text-gray-400">
                      {updateInfo.notes}
                    </p>
                  </div>
                )}
              </div>
              <div className="flex gap-3 pt-2">
                <button
                  onClick={onClose}
                  className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                >
                  Later
                </button>
                <button
                  onClick={handleInstallUpdate}
                  className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600"
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
    <Dialog open={!!activeModal} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent aria-labelledby="modal-system-title" aria-describedby="modal-system-description" className="max-w-2xl">
        {/* Provide hidden, consistent labelling for assistive tech */}
        <DialogHeader className="sr-only">
          <DialogTitle id="modal-system-title">Application modal</DialogTitle>
          <DialogDescription id="modal-system-description">
            Modal content for help, feedback, export, import, or update actions.
          </DialogDescription>
        </DialogHeader>
        {modalContent()}
      </DialogContent>
    </Dialog>
  );
}