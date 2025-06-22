import React from "react";
import { Plus, Send, Square, Trash2 } from "lucide-react";
import {
  AIInput,
  AIInputButton,
  AIInputSubmit,
  AIInputTextarea,
  AIInputToolbar,
  AIInputTools,
} from "@/components/ui/kibo-ui/ai";

interface ChatInputProps {
  query: string;
  isProcessing: boolean;
  canSubmit: boolean;
  onQueryChange: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
  onStop: (e: React.FormEvent) => void;
  onNewChat: () => void;
  onClearConversation: () => void;
}

export const ChatInput = React.memo(function ChatInput({
  query,
  isProcessing,
  canSubmit,
  onQueryChange,
  onSubmit,
  onStop,
  onNewChat,
  onClearConversation,
}: ChatInputProps) {
  return (
    <div className="p-4 bg-background/90 backdrop-blur-sm border-t border-border/30">
      <AIInput onSubmit={isProcessing ? onStop : onSubmit} className="bg-background/80 backdrop-blur-sm border border-border/50 rounded-xl shadow-sm hover:shadow-md transition-all duration-200">
        <AIInputTextarea
          name="message"
          placeholder={
            isProcessing ? "Processing..." : "What would you like to know?"
          }
          value={query}
          onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
            onQueryChange(e.target.value)
          }
          disabled={isProcessing || !canSubmit}
          minHeight={48}
          maxHeight={164}
          className="bg-transparent border-0 focus:ring-0 placeholder:text-muted-foreground text-foreground resize-none"
        />
        <AIInputToolbar className="border-t border-border/30 bg-background/50 backdrop-blur-sm">
          <AIInputTools className="gap-2">
            <AIInputButton
              onClick={onNewChat}
              disabled={isProcessing}
              title="Start new agent chat"
              className="px-3 py-2 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-lg transition-all duration-200 flex items-center gap-2"
            >
              <Plus size={16} />
              New Chat
            </AIInputButton>
            <AIInputButton
              onClick={onClearConversation}
              disabled={isProcessing}
              title="Clear conversation history"
              className="px-3 py-2 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-lg transition-all duration-200 flex items-center gap-2"
            >
              <Trash2 size={16} />
              Clear
            </AIInputButton>
          </AIInputTools>
          <AIInputSubmit
            disabled={!isProcessing && (!canSubmit || !query.trim())}
            variant={isProcessing ? "destructive" : "default"}
            title={isProcessing ? "Stop all operations" : "Submit query"}
            className={`px-4 py-2 rounded-lg font-medium transition-all duration-200 shadow-sm hover:shadow-md ${
              isProcessing
                ? "bg-gradient-to-r from-red-600 to-rose-600 hover:from-red-700 hover:to-rose-700 text-white"
                : "bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700 text-white disabled:opacity-50 disabled:cursor-not-allowed"
            }`}
          >
            {isProcessing ? <Square size={18} /> : <Send size={18} />}
          </AIInputSubmit>
        </AIInputToolbar>
      </AIInput>
    </div>
  );
});
