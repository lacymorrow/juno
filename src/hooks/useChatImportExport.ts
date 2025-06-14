import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChatMessage, ChatExport } from "@/types/app.types";

export const useChatImportExport = () => {
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  const handleExportChat = async (
    conversation: ChatMessage[],
    setConversation: (value: ChatMessage[] | ((prevState: ChatMessage[]) => ChatMessage[])) => void
  ) => {
    setIsExporting(true);
    try {
      console.log("📁 Exporting chat conversation...");

      // Filter out system messages for cleaner export
      const exportableMessages = conversation.filter(
        (msg) => msg.role !== "system"
      );

      const exportData: ChatExport = {
        version: "1.0",
        exported_at: new Date().toISOString(),
        conversation: exportableMessages,
        metadata: {
          total_messages: exportableMessages.length,
          export_type: "full",
        },
      };

      // Use Tauri's save dialog
      const filePath = await invoke("save_chat_export", {
        exportData: JSON.stringify(exportData, null, 2),
        suggestedFilename: `juno-chat-${
          new Date().toISOString().split("T")[0]
        }.json`,
      });

      console.log(`✅ Chat exported to: ${filePath}`);
      setConversation((prev: ChatMessage[]) => [
        ...prev,
        {
          role: "system",
          content: `✅ Chat exported to: ${filePath}`,
          timestamp: Date.now(),
        },
      ]);
    } catch (error) {
      console.error("❌ Failed to export chat:", error);
      setConversation((prev: ChatMessage[]) => [
        ...prev,
        {
          role: "system",
          content: `❌ Failed to export chat: ${error}`,
          timestamp: Date.now(),
        },
      ]);
    } finally {
      setIsExporting(false);
    }
  };

  const handleImportChat = async (
    setConversation: (value: ChatMessage[] | ((prevState: ChatMessage[]) => ChatMessage[])) => void
  ) => {
    setIsImporting(true);
    try {
      console.log("📂 Importing chat conversation...");

      // Use Tauri's open dialog
      const result = await invoke<{ filePath: string; content: string }>(
        "import_chat_file"
      );

      if (!result) {
        console.log("Import cancelled by user");
        return;
      }

      // Parse the imported data
      const importData: ChatExport = JSON.parse(result.content);

      // Validate the import data structure
      if (
        !importData.conversation ||
        !Array.isArray(importData.conversation)
      ) {
        throw new Error("Invalid chat export file format");
      }

      // Restore timestamps for imported messages
      const restoredMessages = importData.conversation.map((msg) => ({
        ...msg,
        timestamp: msg.timestamp || Date.now(),
      }));

      console.log(
        `📂 Imported ${restoredMessages.length} messages from: ${result.filePath}`
      );

      // Replace current conversation with imported messages
      setConversation([
        ...restoredMessages,
        {
          role: "system",
          content: `✅ Imported ${restoredMessages.length} messages from: ${result.filePath}`,
          timestamp: Date.now(),
        },
      ]);
    } catch (error) {
      console.error("❌ Failed to import chat:", error);
      setConversation((prev: ChatMessage[]) => [
        ...prev,
        {
          role: "system",
          content: `❌ Failed to import chat: ${error}`,
          timestamp: Date.now(),
        },
      ]);
    } finally {
      setIsImporting(false);
    }
  };

  return {
    isExporting,
    isImporting,
    handleExportChat,
    handleImportChat,
  };
};