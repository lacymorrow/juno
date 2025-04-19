import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChatMessage, SubmitQueryResult, ServerStatus } from "@/types";

type UseConversationProps = {
  initialConversation: ChatMessage[];
  serverStatus: ServerStatus;
  addLog: (message: string, level?: string) => void;
  playAudioFromBase64: (base64Audio: string) => void;
};

export const useConversation = ({
  initialConversation,
  serverStatus,
  addLog,
  playAudioFromBase64,
}: UseConversationProps) => {
  const [query, setQuery] = useState("");
  const [conversation, setConversation] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const conversationEndRef = useRef<HTMLDivElement>(null);

  // Initialize conversation with initial messages
  useEffect(() => {
    if (initialConversation.length > 0) {
      setConversation(initialConversation);
    }
  }, [initialConversation]);

  // Submit query using Tauri invoke
  const submitQuery = async (text: string) => {
    if (!text.trim() || isProcessing || serverStatus !== "connected") {
      addLog(
        `submission skipped: ${
          !text.trim()
            ? "empty query"
            : isProcessing
            ? "already processing"
            : "server disconnected"
        }`,
        "warn"
      );
      return;
    }

    const userMessage: ChatMessage = { role: "user", content: text };
    setConversation((prev) => [...prev, userMessage]);
    setQuery(""); // Clear input after sending
    addLog(`sending query: ${text}`);
    setIsProcessing(true);

    try {
      // Expecting an object with text and optional audio_base64
      const result: SubmitQueryResult = await invoke("submit_query", {
        query: text,
      });
      addLog("received response from backend", "info");

      const assistantMessage: ChatMessage = {
        role: "assistant",
        content: result.text, // Use the text part of the response
      };
      setConversation((prev) => [...prev, assistantMessage]);

      // Play audio if available
      if (result.audio_base64) {
        addLog("Audio data received, attempting playback.", "info");
        playAudioFromBase64(result.audio_base64);
      } else {
        addLog("No audio data received with the response.", "info");
      }
    } catch (error) {
      addLog(`query invocation error: ${error}`, "error");
      const errorMessage: ChatMessage = {
        role: "system",
        content: `Error processing query: ${error}`,
      };
      setConversation((prev) => [...prev, errorMessage]);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submitQuery(query);
  };

  return {
    query,
    setQuery,
    conversation,
    isProcessing,
    conversationEndRef,
    handleSubmit,
  };
};