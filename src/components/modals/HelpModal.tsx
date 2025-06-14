import { KeyboardShortcuts } from "@/types/app.types";

interface HelpModalProps {
  keyboardShortcuts: KeyboardShortcuts | null;
  onClose: () => void;
}

export const HelpModal = ({ keyboardShortcuts, onClose }: HelpModalProps) => {
  return (
    <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-2xl max-h-[80vh] overflow-y-auto">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-xl font-bold text-gray-900 dark:text-white">
          Juno AI Help & Documentation
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
      <div className="space-y-6 text-gray-800 dark:text-gray-200">
        <section>
          <h3 className="text-lg font-semibold mb-2">🎯 Getting Started</h3>
          <p className="mb-2">
            Juno AI is your intelligent desktop assistant that helps you
            automate tasks, browse the web, and interact with your computer
            using natural language.
          </p>
          <ul className="list-disc list-inside space-y-1">
            <li>Type your requests in the chat interface</li>
            <li>Use voice commands for hands-free operation</li>
            <li>Grant necessary permissions for full functionality</li>
            <li>Customize settings to match your workflow</li>
          </ul>
        </section>
        <section>
          <h3 className="text-lg font-semibold mb-2">⌨️ Keyboard Shortcuts</h3>
          <ul className="list-disc list-inside space-y-1">
            <li>
              <strong>
                {keyboardShortcuts?.agent_mode_toggle || "Option + D"}:
              </strong>{" "}
              Agent Mode Toggle
              <ul className="list-disc list-inside ml-4 text-sm text-gray-600 dark:text-gray-400 mt-1">
                <li>
                  <strong>Tap to Toggle:</strong> Press key to activate agent,
                  press again to stop
                </li>
                <li>
                  <strong>Hold to Activate:</strong> Hold key to activate agent,
                  release to stop
                </li>
              </ul>
            </li>
            <li>
              <strong>
                {keyboardShortcuts?.dictation_input || "Option + Space"}:
              </strong>{" "}
              Toggle Dictation Mode (voice typing)
            </li>
            <li>
              <strong>Wake Words:</strong> Say "Hey Juno" or "Computer" (Always
              Listening Mode)
            </li>
          </ul>
          <p className="text-xs text-gray-500 mt-2">
            Configure agent trigger mode in Settings → General → Agent Trigger
            Mode
          </p>
        </section>
        <section>
          <h3 className="text-lg font-semibold mb-2">💬 Chat Features</h3>
          <ul className="list-disc list-inside space-y-1">
            <li>Type your questions and press Enter</li>
            <li>Use voice commands for hands-free interaction</li>
            <li>Export conversations for backup or sharing</li>
            <li>Import previous conversations to continue</li>
          </ul>
        </section>
        <section>
          <h3 className="text-lg font-semibold mb-2">🛠️ Tools & Automation</h3>
          <ul className="list-disc list-inside space-y-1">
            <li>Screen capture and analysis</li>
            <li>File operations and code analysis</li>
            <li>Web browsing automation</li>
            <li>System control and monitoring</li>
          </ul>
        </section>
        <section>
          <h3 className="text-lg font-semibold mb-2">⚙️ Settings & Permissions</h3>
          <ul className="list-disc list-inside space-y-1">
            <li>Configure accessibility permissions for screen control</li>
            <li>Adjust voice recognition settings</li>
            <li>Customize keyboard shortcuts</li>
            <li>Enable developer tools for advanced features</li>
          </ul>
        </section>
        <div className="pt-4 border-t border-gray-200 dark:border-gray-600">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            For more detailed documentation, visit our GitHub repository or
            contact support.
          </p>
        </div>
      </div>
    </div>
  );
};