import React from "react";

interface ImportModalProps {
  onClose: () => void;
  onImport: () => void;
  isImporting: boolean;
}

export const ImportModal: React.FC<ImportModalProps> = ({
  onClose,
  onImport,
  isImporting,
}) => {
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
            onClick={onImport}
            disabled={isImporting}
            className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50"
          >
            {isImporting ? "Importing..." : "Select File"}
          </button>
        </div>
      </div>
    </div>
  );
};