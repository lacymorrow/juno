import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChatMessage } from "@/types/app.types";

export const useMessageActions = () => {
  const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
  const [savingMessageId, setSavingMessageId] = useState<string | null>(null);

  // Copy and Save handlers for agent responses with enhanced feedback
  const handleCopyResponse = useCallback(
    async (
      content: string, 
      messageIndex: number,
      setConversation: (value: ChatMessage[] | ((prevState: ChatMessage[]) => ChatMessage[])) => void
    ) => {
      const messageId = `copy-${messageIndex}`;
      setCopyingMessageId(messageId);

      try {
        await navigator.clipboard.writeText(content);
        console.log("✅ Copied to clipboard successfully");
        setConversation((prev: ChatMessage[]) => [
          ...prev,
          {
            role: "system",
            content: "✅ Response copied to clipboard",
            timestamp: Date.now(),
          },
        ]);
      } catch (error) {
        console.error("❌ Failed to copy to clipboard:", error);
        setConversation((prev: ChatMessage[]) => [
          ...prev,
          {
            role: "system",
            content: `❌ Failed to copy to clipboard: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        // Clear loading state after a brief delay for visual feedback
        setTimeout(() => setCopyingMessageId(null), 1000);
      }
    },
    []
  );

  const handleSaveResponse = useCallback(
    async (
      content: string,
      format: "html" | "markdown",
      messageIndex: number,
      setConversation: (value: ChatMessage[] | ((prevState: ChatMessage[]) => ChatMessage[])) => void
    ) => {
      const messageId = `save-${format}-${messageIndex}`;
      setSavingMessageId(messageId);

      try {
        console.log(`💾 Saving response as ${format.toUpperCase()}...`);
        const filePath = await invoke("save_agent_response", {
          content,
          format,
          suggested_filename: `agent_response_${Date.now()}`,
        });
        console.log(`✅ Response saved to: ${filePath}`);
        setConversation((prev: ChatMessage[]) => [
          ...prev,
          {
            role: "system",
            content: `✅ Response saved as ${format.toUpperCase()} to: ${filePath}`,
            timestamp: Date.now(),
          },
        ]);
      } catch (error) {
        console.error(`❌ Failed to save response as ${format}:`, error);
        setConversation((prev: ChatMessage[]) => [
          ...prev,
          {
            role: "system",
            content: `❌ Failed to save response as ${format.toUpperCase()}: ${error}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        // Clear loading state after a brief delay for visual feedback
        setTimeout(() => setSavingMessageId(null), 1000);
      }
    },
    []
  );

  return {
    copyingMessageId,
    savingMessageId,
    handleCopyResponse,
    handleSaveResponse,
  };
};