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
    <AIInput onSubmit={isProcessing ? onStop : onSubmit}>
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
      />
      <AIInputToolbar>
        <AIInputTools>
          <AIInputButton
            onClick={onNewChat}
            disabled={isProcessing}
            title="Start new agent chat"
          >
            <Plus size={18} />
            New Chat
          </AIInputButton>
          <AIInputButton
            onClick={onClearConversation}
            disabled={isProcessing}
            title="Clear conversation history"
          >
            <Trash2 size={18} />
            Clear
          </AIInputButton>
        </AIInputTools>
        <AIInputSubmit
          disabled={!isProcessing && (!canSubmit || !query.trim())}
          variant={isProcessing ? "destructive" : "default"}
          title={isProcessing ? "Stop all operations" : "Submit query"}
        >
          {isProcessing ? <Square size={18} /> : <Send size={18} />}
        </AIInputSubmit>
      </AIInputToolbar>
    </AIInput>
  );
});
