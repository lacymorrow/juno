import React from "react";
import { Send, Square } from "lucide-react";

interface ChatInputProps {
  query: string;
  setQuery: (query: string) => void;
  isProcessing: boolean;
  serverStatus: "checking" | "connected" | "error";
  onSubmit: (text: string) => void;
}

export const ChatInput: React.FC<ChatInputProps> = ({
  query,
  setQuery,
  isProcessing,
  serverStatus,
  onSubmit,
}) => {
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (query.trim() && !isProcessing && serverStatus === "connected") {
      onSubmit(query.trim());
    }
  };

  const isDisabled = isProcessing || serverStatus !== "connected";

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 p-4">
      <form onSubmit={handleSubmit} className="flex gap-2">
        <div className="flex-1 relative">
          <textarea
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSubmit(e);
              }
            }}
            placeholder={
              serverStatus === "checking"
                ? "Connecting to server..."
                : serverStatus === "error"
                ? "Server not connected"
                : isProcessing
                ? "Processing..."
                : "Type your message here... (Shift+Enter for new line)"
            }
            disabled={isDisabled}
            className="w-full px-3 py-2 pr-10 border border-gray-300 dark:border-gray-600 rounded-lg 
                     focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400 
                     bg-white dark:bg-gray-800 text-gray-900 dark:text-white resize-none
                     disabled:opacity-50 disabled:cursor-not-allowed"
            rows={1}
            style={{
              minHeight: "40px",
              maxHeight: "120px",
              height: "auto",
            }}
          />
          
          {/* Server status indicator */}
          <div className="absolute right-12 top-1/2 transform -translate-y-1/2">
            <div
              className={`w-2 h-2 rounded-full ${
                serverStatus === "connected"
                  ? "bg-green-500"
                  : serverStatus === "checking"
                  ? "bg-yellow-500 animate-pulse"
                  : "bg-red-500"
              }`}
              title={
                serverStatus === "connected"
                  ? "Connected to server"
                  : serverStatus === "checking"
                  ? "Checking server connection"
                  : "Server not connected"
              }
            />
          </div>
        </div>
        
        <button
          type="submit"
          disabled={isDisabled || !query.trim()}
          className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 
                   focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 
                   disabled:cursor-not-allowed transition-colors flex items-center gap-2"
        >
          {isProcessing ? (
            <>
              <Square className="w-4 h-4" />
              Stop
            </>
          ) : (
            <>
              <Send className="w-4 h-4" />
              Send
            </>
          )}
        </button>
      </form>
      
      {/* Help text */}
      <div className="mt-2 text-xs text-gray-500 dark:text-gray-400 flex justify-between">
        <span>Press Enter to send, Shift+Enter for new line</span>
        <span>
          Status: {serverStatus === "connected" ? "🟢 Connected" : 
                   serverStatus === "checking" ? "🟡 Connecting..." : 
                   "🔴 Disconnected"}
        </span>
      </div>
    </div>
  );
};