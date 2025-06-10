import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService";
import { base64ToBlob } from "@/lib/utils/chat";
import type { ChatMessage, ChatExport, FeedbackData, UpdateInfo } from "@/types/chat";

// Audio playback service
export const playAudioFromBase64 = (base64Audio: string) => {
  try {
    console.log("🔊 Playing audio from base64...");
    
    // Stop any current TTS
    stopTTS();

    const audioBlob = base64ToBlob(base64Audio);
    const audioUrl = URL.createObjectURL(audioBlob);
    const audio = new Audio(audioUrl);

    // Set this as the current audio element for TTS management
    setCurrentAudioElement(audio);

    // Clean up on end
    audio.addEventListener("ended", () => {
      console.log("🔊 Audio playback finished");
      URL.revokeObjectURL(audioUrl);
      setCurrentAudioElement(null);
    });

    // Clean up on error
    audio.addEventListener("error", (e) => {
      console.error("🔊 Audio playback error:", e);
      URL.revokeObjectURL(audioUrl);
      setCurrentAudioElement(null);
    });

    // Play the audio
    audio.play().catch((error) => {
      console.error("🔊 Failed to play audio:", error);
      URL.revokeObjectURL(audioUrl);
      setCurrentAudioElement(null);
    });
  } catch (error) {
    console.error("🔊 Error setting up audio playback:", error);
  }
};

// Chat export service - using backend command
export const exportChat = async (conversation: ChatMessage[]) => {
  try {
    if (conversation.length === 0) {
      toast.error("No conversation to export");
      return;
    }

    console.log("📤 Starting chat export...");

    const exportData: ChatExport = {
      version: "1.0",
      exported_at: new Date().toISOString(),
      conversation: conversation.filter((msg) => msg.role !== "system"), // Exclude system messages
      metadata: {
        total_messages: conversation.length,
        export_type: "filtered",
      },
    };

    // Use backend command to handle file save dialog and writing
    const result = (await invoke("save_chat_export", {
      data: JSON.stringify(exportData, null, 2),
    })) as { success: boolean; path?: string; error?: string };

    if (result.success && result.path) {
      console.log("✅ Chat exported successfully to:", result.path);
      toast.success("Chat exported successfully!");
      return result.path;
    } else {
      throw new Error(result.error || "Export failed");
    }
  } catch (error) {
    console.error("❌ Failed to export chat:", error);
    toast.error(`Failed to export chat: ${error}`);
    throw error;
  }
};

// Chat import service - using backend command
export const importChat = async (): Promise<{ messages: ChatMessage[]; messageCount: number }> => {
  try {
    console.log("📥 Starting chat import...");

    // Use backend command to handle file open dialog and reading
    const result = (await invoke("load_chat_import")) as {
      success: boolean;
      data?: string;
      error?: string;
      messageCount?: number;
    };

    if (result.success && result.data) {
      const importData: ChatExport = JSON.parse(result.data);

      // Validate import format
      if (!importData.conversation || !Array.isArray(importData.conversation)) {
        throw new Error("Invalid chat export format");
      }

      // Add timestamps to imported messages if missing
      const importedMessages = importData.conversation.map((msg) => ({
        ...msg,
        timestamp: msg.timestamp || Date.now(),
      }));

      console.log("✅ Chat imported successfully:", importData);
      toast.success(`Chat imported successfully! ${importedMessages.length} messages loaded.`);

      return {
        messages: importedMessages,
        messageCount: result.messageCount || importedMessages.length,
      };
    } else {
      if (result.error && !result.error.includes("cancelled")) {
        throw new Error(result.error);
      }
      // User cancelled - return empty result
      throw new Error("Import cancelled");
    }
  } catch (error) {
    console.error("❌ Failed to import chat:", error);
    const errorMessage = error instanceof Error ? error.message : String(error);
    if (!errorMessage.includes("cancelled")) {
      toast.error(`Failed to import chat: ${errorMessage}`);
    }
    throw error;
  }
};

// Copy response service
export const copyToClipboard = async (content: string): Promise<void> => {
  try {
    await navigator.clipboard.writeText(content);
    console.log("✅ Copied to clipboard successfully");
    toast.success("Response copied to clipboard");
  } catch (error) {
    console.error("❌ Failed to copy to clipboard:", error);
    toast.error("Failed to copy to clipboard");
    throw error;
  }
};

// Save response service
export const saveResponse = async (
  content: string,
  format: "html" | "markdown"
): Promise<string> => {
  try {
    console.log(`💾 Saving response as ${format.toUpperCase()}...`);
    const filePath = await invoke<string>("save_agent_response", {
      content,
      format,
      suggested_filename: `agent_response_${Date.now()}`,
    });
    console.log(`✅ Response saved to: ${filePath}`);
    toast.success(`Response saved as ${format.toUpperCase()}`);
    return filePath;
  } catch (error) {
    console.error(`❌ Failed to save response as ${format}:`, error);
    toast.error(`Failed to save response as ${format.toUpperCase()}`);
    throw error;
  }
};

// Feedback submission service
export const submitFeedback = async (feedbackData: FeedbackData): Promise<void> => {
  try {
    if (!feedbackData.title.trim() || !feedbackData.description.trim()) {
      throw new Error("Please fill in both title and description fields.");
    }

    console.log("📝 Submitting feedback:", feedbackData);

    // Create GitHub issue URL or mailto link for feedback
    if (feedbackData.type === "issue") {
      const title = encodeURIComponent(feedbackData.title);
      const body = encodeURIComponent(
        `**Priority:** ${feedbackData.priority}\n\n**Description:**\n${
          feedbackData.description
        }\n\n**Contact:** ${feedbackData.email || "Not provided"}`
      );
      const githubUrl = `https://github.com/lacymorrow/juno/issues/new?title=${title}&body=${body}`;

      // Open GitHub issues page
      await invoke("open_url", { url: githubUrl });
    } else {
      // For general feedback, create mailto link
      const subject = encodeURIComponent(`Juno AI Feedback: ${feedbackData.title}`);
      const body = encodeURIComponent(
        `Priority: ${feedbackData.priority}\n\nDescription:\n${feedbackData.description}`
      );
      const mailtoUrl = `mailto:feedback@juno-ai.com?subject=${subject}&body=${body}`;

      await invoke("open_url", { url: mailtoUrl });
    }

    console.log("✅ Feedback submission initiated");
    toast.success("Feedback form opened. Thank you for your input!");
  } catch (error) {
    console.error("❌ Failed to submit feedback:", error);
    toast.error(`Failed to open feedback form: ${error}`);
    throw error;
  }
};

// Update check service
export const checkForUpdates = async (): Promise<UpdateInfo> => {
  try {
    console.log("🔄 Checking for updates...");
    
    // Placeholder implementation - replace with actual update check
    const updateInfo: UpdateInfo = await invoke("check_for_updates");
    
    console.log("✅ Update check completed", updateInfo);
    return updateInfo;
  } catch (error) {
    console.error("❌ Failed to check for updates:", error);
    toast.error("Failed to check for updates");
    throw error;
  }
};

// Update installation service
export const installUpdate = async (): Promise<void> => {
  try {
    console.log("📦 Installing update...");
    
    await invoke("install_update");
    
    console.log("✅ Update installation initiated");
    toast.success("Update installation started! Application will restart.");
  } catch (error) {
    console.error("❌ Failed to install update:", error);
    toast.error("Failed to install update");
    throw error;
  }
};