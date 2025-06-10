import React from "react";

interface HelpModalProps {
  onClose: () => void;
}

export const HelpModal: React.FC<HelpModalProps> = ({ onClose }) => {
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
              <strong>Option + D:</strong> Toggle Agent Mode (AI
              conversations)
            </li>
            <li>
              <strong>Option + Space:</strong> Toggle Dictation Mode
              (voice typing)
            </li>
            <li>
              <strong>Wake Words:</strong> Say "Hey Juno" or "Computer"
              (Always Listening Mode)
            </li>
          </ul>
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
};