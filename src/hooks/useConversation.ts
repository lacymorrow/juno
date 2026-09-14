import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage, ResponseExportInput } from "@/types/chat";
import { COMMANDS, LIMITS } from "@/lib/constants.generated";

/** How long the copy button shows its check before turning back into the copy glyph. */
const COPIED_CHECK_MS = 1500;

/** Where the share sheet drops down from: the Share button's box in CSS px, relative to the web view. */
export interface ShareAnchor {
    x: number;
    y: number;
    width: number;
    height: number;
}

export function useConversation() {
    const [conversation, setConversation] = useState<ChatMessage[]>([]);
    const [query, setQuery] = useState("");
    const copiedResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
        return () => {
            if (copiedResetTimer.current) clearTimeout(copiedResetTimer.current);
        };
    }, []);

    // Conversation pruning function with memory optimization
    const pruneConversationIfNeeded = useCallback((messages: ChatMessage[]): ChatMessage[] => {
        const maxMessages = LIMITS.MAX_CHAT_HISTORY_ITEMS;
        const minMessagesToKeep = Math.max(50, maxMessages * 0.3); // Keep at least 30% of limit

        if (messages.length <= maxMessages) {
            return messages;
        }

        console.log(`Pruning conversation: ${messages.length} -> ${minMessagesToKeep} messages`);

        // Always keep the most recent messages, but try to preserve message pairs
        const messagesToKeep = Math.floor(minMessagesToKeep);
        const prunedMessages = messages.slice(-messagesToKeep);

        // Add a system message indicating pruning occurred
        const pruningNotice: ChatMessage = {
            role: "system",
            content: `[Conversation pruned - keeping last ${messagesToKeep} messages for performance]`,
            timestamp: Date.now(),
        };

        return [pruningNotice, ...prunedMessages];
    }, []);

    // Enhanced setConversation wrapper with automatic pruning
    const setConversationWithPruning = useCallback((
        updateFn: React.SetStateAction<ChatMessage[]>
    ) => {
        setConversation(prevConversation => {
            const newConversation = typeof updateFn === 'function'
                ? updateFn(prevConversation)
                : updateFn;

            // Apply pruning if needed
            return pruneConversationIfNeeded(newConversation);
        });
    }, [pruneConversationIfNeeded]);

    // Function to start a new chat (clear conversation and reset input)
    const startNewChat = useCallback(() => {
        console.log("Starting new chat - clearing conversation and input");
        setConversation([]); // Clear conversation
        setQuery(""); // Clear input field
    }, []);

    // Add a system message to conversation
    const addSystemMessage = useCallback((content: string) => {
        const systemMessage: ChatMessage = {
            role: "system",
            content,
            timestamp: Date.now(),
        };
        setConversationWithPruning(prev => [...prev, systemMessage]);
    }, [setConversationWithPruning]);

    // Add a user message to conversation
    const addUserMessage = useCallback((content: string) => {
        const userMessage: ChatMessage = {
            role: "user",
            content,
            timestamp: Date.now(),
        };
        setConversationWithPruning(prev => [...prev, userMessage]);
    }, [setConversationWithPruning]);

    // Add an assistant message to conversation
    const addAssistantMessage = useCallback((content: string, metadata?: Partial<ChatMessage>) => {
        const assistantMessage: ChatMessage = {
            role: "assistant",
            content,
            timestamp: Date.now(),
            ...metadata,
        };
        setConversationWithPruning(prev => [...prev, assistantMessage]);
    }, [setConversationWithPruning]);

    // Copy a response through the native pasteboard (navigator.clipboard is
    // refused in the floating bar, which is never the key window). The button
    // shows a check for a moment on success; a failure gets one system line
    // since there is nothing else to show.
    const handleCopyResponse = useCallback(
        async (response: ResponseExportInput, messageIndex: number, onCopiedChange: (id: string | null) => void) => {
            try {
                await invoke(COMMANDS.AGENT_COPY_AGENT_RESPONSE, { response });
            } catch (error) {
                console.error("Failed to copy to clipboard:", error);
                addSystemMessage(`Couldn't copy the response: ${error}`);
                return;
            }
            onCopiedChange(`copy-${messageIndex}`);
            if (copiedResetTimer.current) clearTimeout(copiedResetTimer.current);
            copiedResetTimer.current = setTimeout(() => onCopiedChange(null), COPIED_CHECK_MS);
        },
        [addSystemMessage]
    );

    // Open the native share sheet for a response, dropping down from the
    // Share button. Saving as Markdown/HTML lives inside the sheet.
    const handleShareResponse = useCallback(
        async (response: ResponseExportInput, anchor: ShareAnchor) => {
            try {
                await invoke(COMMANDS.AGENT_SHARE_AGENT_RESPONSE, { response, anchor });
            } catch (error) {
                console.error("Failed to open the share sheet:", error);
                addSystemMessage(`Couldn't open the share sheet: ${error}`);
            }
        },
        [addSystemMessage]
    );

    // Update conversation (for import functionality)
    const updateConversation = useCallback((newMessages: ChatMessage[]) => {
        setConversation(newMessages);
    }, []);

    return {
        // State
        conversation,
        query,

        // Basic actions
        setConversation,
        setConversationWithPruning,
        setQuery,
        startNewChat,
        updateConversation,

        // Message operations
        addSystemMessage,
        addUserMessage,
        addAssistantMessage,

        // Enhanced operations
        handleCopyResponse,
        handleShareResponse,

        // Utilities
        pruneConversationIfNeeded,
    };
}
