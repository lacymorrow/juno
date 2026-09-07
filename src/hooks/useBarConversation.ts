import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useConversation } from "@/hooks/useConversation";
import { useBackendEvents } from "@/hooks/useBackendEvents";

const noop = () => {};

/**
 * The floating bar's view of the conversation.
 *
 * Same messages, same events, same reducers as the main window
 * (`useConversation` + `useBackendEvents`), so a query started anywhere —
 * typed into the bar, spoken, sent from the main window, a cloud client or a
 * rendered component — shows up in the bar's chat pane identically.
 *
 * Two deliberate differences from the main window:
 *  - audio playback is a no-op here, so a spoken response is never played
 *    twice (the main window owns TTS playback);
 *  - the backend health probe is skipped, so the pane never opens on a
 *    "Connected…" system message.
 */
export function useBarConversation() {
  const conversation = useConversation();
  const [isProcessing, setIsProcessing] = useState(false);
  const [copyingMessageId, setCopyingMessageId] = useState<string | null>(null);
  const [savingMessageId, setSavingMessageId] = useState<string | null>(null);

  useBackendEvents({
    addSystemMessage: conversation.addSystemMessage,
    addAssistantMessage: conversation.addAssistantMessage,
    setConversationWithPruning: conversation.setConversationWithPruning,
    playAudioFromBase64: noop,
    stopCurrentAudio: noop,
    setIsProcessing,
    setServerStatus: noop,
    skipServerCheck: true,
  });

  const handleCopyResponse = useCallback(
    (content: string, messageIndex: number) =>
      conversation.handleCopyResponse(content, messageIndex, setCopyingMessageId),
    [conversation.handleCopyResponse],
  );

  const handleSaveResponse = useCallback(
    (content: string, format: "html" | "markdown", messageIndex: number) =>
      conversation.handleSaveResponse(content, format, messageIndex, setSavingMessageId),
    [conversation.handleSaveResponse],
  );

  const handleApprovalUpdate = useCallback(
    (toolId: string, state: "approved" | "denied") => {
      conversation.setConversationWithPruning((prev) =>
        prev.map((msg) =>
          msg.tool_id === toolId ? { ...msg, approval_state: state } : msg,
        ),
      );
    },
    [conversation.setConversationWithPruning],
  );

  const handleContinuationUpdate = useCallback(
    (requestId: string, state: "stopped" | "continued") => {
      conversation.setConversationWithPruning((prev) =>
        prev.map((msg) =>
          msg.continuation_request_id === requestId
            ? { ...msg, continuation_state: state }
            : msg,
        ),
      );
    },
    [conversation.setConversationWithPruning],
  );

  // Same command the main window's stop button uses; the backend fans the
  // stop out to every window and the bar state machine.
  const stop = useCallback(async () => {
    try {
      await invoke("stop_all_operations");
    } catch (error) {
      console.error("FloatingBar: failed to stop operations:", error);
    }
  }, []);

  return {
    messages: conversation.conversation,
    isProcessing,
    startNewChat: conversation.startNewChat,
    copyingMessageId,
    savingMessageId,
    handleCopyResponse,
    handleSaveResponse,
    handleApprovalUpdate,
    handleContinuationUpdate,
    stop,
  };
}
