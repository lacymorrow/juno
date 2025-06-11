import React, { useEffect } from "react";
import { Copy, Save, Info } from "lucide-react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage } from "@/types/chat";
import { 
  shouldShowTimestamp, 
  formatMessageTimestamp, 
  formatFullTimestamp 
} from "@/lib/utils/chat";
import { 
  isScreenshotTool, 
  isFileOperationTool, 
  isBrowserTool, 
  isSystemTool, 
  getFriendlyToolName,
  getNotificationClassName 
} from "@/lib/utils/tools";

interface MessageListProps {
  conversation: ChatMessage[];
  copyingMessageId: string | null;
  setCopyingMessageId: (id: string | null) => void;
  savingMessageId: string | null;
  setSavingMessageId: (id: string | null) => void;
  conversationEndRef: React.RefObject<HTMLDivElement>;
}

export const MessageList: React.FC<MessageListProps> = ({
  conversation,
  copyingMessageId,
  setCopyingMessageId,
  savingMessageId,
  setSavingMessageId,
  conversationEndRef,
}) => {
  // Auto-scroll to bottom when conversation updates
  useEffect(() => {
    conversationEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [conversation, conversationEndRef]);

  // Copy message content
  const copyToClipboard = async (content: string, messageIndex: number) => {
    const messageId = `message-${messageIndex}`;
    setCopyingMessageId(messageId);
    
    try {
      await navigator.clipboard.writeText(content);
      toast.success("Copied to clipboard");
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
      toast.error("Failed to copy to clipboard");
    } finally {
      setTimeout(() => setCopyingMessageId(null), 1000);
    }
  };

  // Save message to file
  const saveToFile = async (content: string, messageIndex: number) => {
    const messageId = `message-${messageIndex}`;
    setSavingMessageId(messageId);
    
    try {
      await invoke("save_agent_response", { content });
      toast.success("Saved to file");
    } catch (error) {
      console.error("Failed to save to file:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      if (!errorMessage.includes("cancelled")) {
        toast.error(`Failed to save to file: ${errorMessage}`);
      }
    } finally {
      setTimeout(() => setSavingMessageId(null), 1000);
    }
  };

  // Render JSX content safely
  const renderJsxContent = (content: string) => {
    try {
      const processedContent = content
        .replace(/className=/g, "class=")
        .replace(/onClick=/g, "onclick=")
        .replace(/onChange=/g, "onchange=");
      
      return <div dangerouslySetInnerHTML={{ __html: processedContent }} />;
    } catch (error) {
      console.error("Error rendering JSX content:", error);
      return <pre className="whitespace-pre-wrap text-sm">{content}</pre>;
    }
  };

  // Get message content based on role and type
  const getMessageContent = (message: ChatMessage) => {
    if (message.isJsx && message.role === "assistant") {
      return renderJsxContent(message.content);
    }
    return <pre className="whitespace-pre-wrap text-sm font-mono">{message.content}</pre>;
  };

  // Render tool call messages with enhanced styling
  const renderToolMessage = (message: ChatMessage, index: number) => {
    const isRequest = message.role === "tool_call_request";
    const isResult = message.role === "tool_call_result";
    const toolName = message.tool_name || "unknown";
    const friendlyName = getFriendlyToolName(toolName);
    
    // Determine notification class based on tool and success
    const notificationClass = getNotificationClassName(
      isScreenshotTool(toolName) ? "screenshot" :
      isFileOperationTool(toolName) ? "file" :
      isBrowserTool(toolName) ? "browser" :
      isSystemTool(toolName) ? "system" : "general",
      isResult ? message.success : undefined
    );

    return (
      <div className={`p-3 rounded-lg border-l-4 ${notificationClass}`}>
        <div className="flex items-center gap-2 mb-2">
          <Info className="w-4 h-4" />
          <span className="font-medium text-sm">
            {isRequest ? "🔧 Calling Tool" : isResult ? "✅ Tool Result" : "🔧 Tool Event"}
          </span>
          <span className="text-xs bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded">
            {friendlyName}
          </span>
          {isResult && (
            <span className={`text-xs px-2 py-1 rounded ${
              message.success 
                ? "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200" 
                : "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200"
            }`}>
              {message.success ? "Success" : "Failed"}
            </span>
          )}
        </div>
        
        {message.content && (
          <div className="text-sm mb-2">
            {getMessageContent(message)}
          </div>
        )}
        
        {message.tool_args && (
          <details className="text-xs">
            <summary className="cursor-pointer text-gray-600 dark:text-gray-400 mb-1">
              Arguments
            </summary>
            <pre className="bg-gray-50 dark:bg-gray-800 p-2 rounded overflow-x-auto">
              {JSON.stringify(message.tool_args, null, 2)}
            </pre>
          </details>
        )}
        
        {message.tool_output && (
          <details className="text-xs">
            <summary className="cursor-pointer text-gray-600 dark:text-gray-400 mb-1">
              Output
            </summary>
            <pre className="bg-gray-50 dark:bg-gray-800 p-2 rounded overflow-x-auto">
              {JSON.stringify(message.tool_output, null, 2)}
            </pre>
          </details>
        )}
        
        {message.screenshot_base64 && (
          <div className="mt-2">
            <img
              src={`data:image/png;base64,${message.screenshot_base64}`}
              alt="Tool screenshot"
              className="max-w-full h-auto rounded border"
            />
          </div>
        )}
        
        <MessageActions
          content={message.content}
          messageIndex={index}
          copyingMessageId={copyingMessageId}
          savingMessageId={savingMessageId}
          onCopy={copyToClipboard}
          onSave={saveToFile}
        />
      </div>
    );
  };

  // Regular message rendering
  const renderRegularMessage = (message: ChatMessage, index: number) => {
    const isUser = message.role === "user";
    const isSystem = message.role === "system";
    const isThinking = message.role === "thinking";
    
    const messageClass = isUser
      ? "bg-blue-500 text-white ml-auto"
      : isSystem
      ? "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200 border border-yellow-300 dark:border-yellow-600"
      : isThinking
      ? "bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200 border border-purple-300 dark:border-purple-600 italic"
      : "bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100";

    return (
      <div className={`p-3 rounded-lg max-w-[85%] ${messageClass}`}>
        <div className="flex flex-col gap-2">
          {getMessageContent(message)}
          
          {message.screenshot_base64 && (
            <div className="mt-2">
              <img
                src={`data:image/png;base64,${message.screenshot_base64}`}
                alt="Assistant screenshot"
                className="max-w-full h-auto rounded border"
              />
            </div>
          )}
          
          {!isUser && !isSystem && !isThinking && (
            <MessageActions
              content={message.content}
              messageIndex={index}
              copyingMessageId={copyingMessageId}
              savingMessageId={savingMessageId}
              onCopy={copyToClipboard}
              onSave={saveToFile}
            />
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-4">
      {conversation.map((message, index) => {
        const previousMessage = index > 0 ? conversation[index - 1] : null;
        const showTimestamp = shouldShowTimestamp(message, previousMessage);
        
        return (
          <div key={index} className="space-y-2">
            {showTimestamp && message.timestamp && (
              <div className="flex justify-center">
                <span
                  className="text-xs text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded-full"
                  title={formatFullTimestamp(message.timestamp)}
                >
                  {formatMessageTimestamp(message.timestamp)}
                </span>
              </div>
            )}
            
            <div className={`flex ${message.role === "user" ? "justify-end" : "justify-start"}`}>
              {message.role === "tool_call_request" || message.role === "tool_call_result"
                ? renderToolMessage(message, index)
                : renderRegularMessage(message, index)}
            </div>
          </div>
        );
      })}
      <div ref={conversationEndRef} />
    </div>
  );
};

// Message actions component
interface MessageActionsProps {
  content: string;
  messageIndex: number;
  copyingMessageId: string | null;
  savingMessageId: string | null;
  onCopy: (content: string, index: number) => void;
  onSave: (content: string, index: number) => void;
}

const MessageActions: React.FC<MessageActionsProps> = ({
  content,
  messageIndex,
  copyingMessageId,
  savingMessageId,
  onCopy,
  onSave,
}) => {
  const messageId = `message-${messageIndex}`;
  const isCopying = copyingMessageId === messageId;
  const isSaving = savingMessageId === messageId;

  return (
    <div className="flex gap-2 mt-2 opacity-0 group-hover:opacity-100 transition-opacity">
      <button
        onClick={() => onCopy(content, messageIndex)}
        disabled={isCopying}
        className="p-1 rounded text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors disabled:opacity-50"
        title="Copy message"
      >
        <Copy className="w-4 h-4" />
      </button>
      
      <button
        onClick={() => onSave(content, messageIndex)}
        disabled={isSaving}
        className="p-1 rounded text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors disabled:opacity-50"
        title="Save to file"
      >
        <Save className="w-4 h-4" />
      </button>
    </div>
  );
};