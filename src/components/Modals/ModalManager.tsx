import React from "react";
import type { ModalType, FeedbackData, UpdateInfo, ChatMessage } from "@/types/chat";
import { HelpModal } from "./HelpModal";
import { FeedbackModal } from "./FeedbackModal";
import { ExportModal } from "./ExportModal";
import { ImportModal } from "./ImportModal";
import { UpdateModal } from "./UpdateModal";

interface ModalManagerProps {
  activeModal: ModalType;
  onClose: () => void;
  // Feedback modal props
  feedbackData: FeedbackData;
  setFeedbackData: React.Dispatch<React.SetStateAction<FeedbackData>>;
  onSubmitFeedback: () => void;
  // Export modal props
  conversation: ChatMessage[];
  onExportChat: () => void;
  isExporting: boolean;
  // Import modal props
  onImportChat: () => void;
  isImporting: boolean;
  // Update modal props
  updateInfo: UpdateInfo | null;
  onInstallUpdate: () => void;
}

export const ModalManager: React.FC<ModalManagerProps> = ({
  activeModal,
  onClose,
  feedbackData,
  setFeedbackData,
  onSubmitFeedback,
  conversation,
  onExportChat,
  isExporting,
  onImportChat,
  isImporting,
  updateInfo,
  onInstallUpdate,
}) => {
  if (!activeModal) return null;

  const renderModalContent = () => {
    switch (activeModal) {
      case "help":
        return <HelpModal onClose={onClose} />;
      case "feedback":
        return (
          <FeedbackModal
            onClose={onClose}
            feedbackData={feedbackData}
            setFeedbackData={setFeedbackData}
            onSubmit={onSubmitFeedback}
          />
        );
      case "export":
        return (
          <ExportModal
            onClose={onClose}
            conversation={conversation}
            onExport={onExportChat}
            isExporting={isExporting}
          />
        );
      case "import":
        return (
          <ImportModal
            onClose={onClose}
            onImport={onImportChat}
            isImporting={isImporting}
          />
        );
      case "update":
        return (
          <UpdateModal
            onClose={onClose}
            updateInfo={updateInfo}
            onInstall={onInstallUpdate}
          />
        );
      default:
        return null;
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      {renderModalContent()}
    </div>
  );
};