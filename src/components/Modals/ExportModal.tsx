import React from "react";
import type { ChatMessage } from "@/types/chat";

interface ExportModalProps {
  onClose: () => void;
  conversation: ChatMessage[];
  onExport: () => void;
  isExporting: boolean;
}

export const ExportModal: React.FC<ExportModalProps> = ({
  onClose,
  conversation,
  onExport,
  isExporting,
}) => {
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
            onClick={onExport}
            disabled={isExporting}
            className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600 disabled:opacity-50"
          >
            {isExporting ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
};