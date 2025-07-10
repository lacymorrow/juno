import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage } from "@/components/ChatMessage";
import { LIMITS } from "@/lib/constants.generated";

export function useConversation() {
    const [conversation, setConversation] = useState<ChatMessage[]>([]);
    const [query, setQuery] = useState("");

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

    // Update or add a streaming assistant message
    const addOrUpdateStreamingMessage = useCallback((messageId: string, content: string, isComplete: boolean = false) => {
        setConversationWithPruning(prev => {
            // Find existing streaming message with this ID
            const existingIndex = prev.findIndex(msg => 
                msg.role === "assistant" && 
                msg.messageId === messageId
            );
            
            if (existingIndex >= 0) {
                // Update existing streaming message
                const updatedMessages = [...prev];
                updatedMessages[existingIndex] = {
                    ...updatedMessages[existingIndex],
                    content,
                    isStreaming: !isComplete,
                    messageId: messageId,
                };
                return updatedMessages;
            } else {
                // Add new streaming message
                const streamingMessage: ChatMessage = {
                    role: "assistant",
                    content,
                    timestamp: Date.now(),
                    isStreaming: !isComplete,
                    messageId: messageId,
                };
                return [...prev, streamingMessage];
            }
        });
    }, [setConversationWithPruning]);

    // Copy response handler with enhanced feedback
    const handleCopyResponse = useCallback(
        async (content: string, messageIndex: number, onCopyingStateChange: (id: string | null) => void) => {
            const messageId = `copy-${messageIndex}`;
            onCopyingStateChange(messageId);

            try {
                await navigator.clipboard.writeText(content);
                console.log("✅ Copied to clipboard successfully");
                addSystemMessage("✅ Response copied to clipboard");
            } catch (error) {
                console.error("❌ Failed to copy to clipboard:", error);
                addSystemMessage(`❌ Failed to copy to clipboard: ${error}`);
            } finally {
                // Clear loading state after a brief delay for visual feedback
                setTimeout(() => onCopyingStateChange(null), 1000);
            }
        },
        [addSystemMessage]
    );

    // Save response handler with enhanced feedback
    const handleSaveResponse = useCallback(
        async (
            content: string,
            format: "html" | "markdown",
            messageIndex: number,
            onSavingStateChange: (id: string | null) => void
        ) => {
            const messageId = `save-${format}-${messageIndex}`;
            onSavingStateChange(messageId);

            try {
                console.log(`💾 Saving response as ${format.toUpperCase()}...`);
                const filePath = await invoke("save_agent_response", {
                    content,
                    format,
                    suggested_filename: `agent_response_${Date.now()}`,
                });
                console.log(`✅ Response saved to: ${filePath}`);
                addSystemMessage(`✅ Response saved as ${format.toUpperCase()} to: ${filePath}`);
            } catch (error) {
                console.error(`❌ Failed to save response as ${format}:`, error);
                addSystemMessage(`❌ Failed to save response as ${format.toUpperCase()}: ${error}`);
            } finally {
                // Clear loading state after a brief delay for visual feedback
                setTimeout(() => onSavingStateChange(null), 1000);
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
        addOrUpdateStreamingMessage,

        // Enhanced operations
        handleCopyResponse,
        handleSaveResponse,

        // Utilities
        pruneConversationIfNeeded,
    };
}
