import type { ResponseExportInput } from "@/types/chat";
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useConversation } from "@/hooks/useConversation";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import type { BackendStatus } from "@/components/ExamplePrompts";

const noop = () => {};

/**
 * The floating bar's view of the conversation.
 *
 * Same messages, same events, same reducers as the main window
 * (`useConversation` + `useBackendEvents`), so a query started anywhere —
 * typed into the bar, spoken, sent from the main window, a cloud client or a
 * rendered component — shows up in the bar's chat pane identically.
 *
 * One deliberate difference from the main window: the backend health probe in
 * `useBackendEvents` is skipped, so the pane never carries a "Connected…"
 * system message. The bar asks the same question itself, quietly, so the
 * example prompts know when a click can be sent.
 *
 * Speech needs no such guard. Rust owns playback end to end (one `afplay`
 * child process), so a spoken response cannot double up no matter how many
 * windows are mirroring the conversation.
 */
export function useBarConversation() {
  const conversation = useConversation();
  const [isProcessing, setIsProcessing] = useState(false);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const [serverStatus, setServerStatus] = useState<BackendStatus>("connecting");
  const hasCheckedServer = useRef(false);

  useBackendEvents({
    addSystemMessage: conversation.addSystemMessage,
    addAssistantMessage: conversation.addAssistantMessage,
    setConversationWithPruning: conversation.setConversationWithPruning,
    setIsProcessing,
    setServerStatus: noop,
    skipServerCheck: true,
  });

  // Same command the main window's probe uses, without its system message.
  // The ref keeps React Strict Mode's double effect to one call.
  useEffect(() => {
    if (hasCheckedServer.current) return;
    hasCheckedServer.current = true;
    invoke<{ backend_running: boolean }>("check_server_status")
      .then((status) => setServerStatus(status?.backend_running ? "connected" : "error"))
      .catch(() => setServerStatus("error"));
  }, []);

  const handleCopyResponse = useCallback(
    (response: ResponseExportInput, messageIndex: number) =>
      conversation.handleCopyResponse(response, messageIndex, setCopiedMessageId),
    [conversation.handleCopyResponse],
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
    serverStatus,
    startNewChat: conversation.startNewChat,
    copiedMessageId,
    handleCopyResponse,
    handleShareResponse: conversation.handleShareResponse,
    handleApprovalUpdate,
    handleContinuationUpdate,
    stop,
  };
}
