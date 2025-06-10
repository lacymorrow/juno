import React from "react";
import type { UpdateInfo } from "@/types/chat";

interface UpdateModalProps {
  onClose: () => void;
  updateInfo: UpdateInfo | null;
  onInstall: () => void;
}

export const UpdateModal: React.FC<UpdateModalProps> = ({
  onClose,
  updateInfo,
  onInstall,
}) => {
  if (!updateInfo) return null;

  return (
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
            onClick={onInstall}
            className="flex-1 px-4 py-2 bg-green-500 text-white rounded-md hover:bg-green-600"
          >
            Install Update
          </button>
        </div>
      </div>
    </div>
  );
};