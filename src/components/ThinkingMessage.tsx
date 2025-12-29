import { cn } from "@/lib/utils";
import { ChevronDownIcon } from "lucide-react";
import { useState, useEffect, useRef } from "react";

interface ThinkingMessageProps {
  content: string;
  timestamp?: number;
  isStreaming?: boolean;
}

export function ThinkingMessage({ content, isStreaming = false }: ThinkingMessageProps) {
  // Start expanded if streaming, collapsed otherwise
  const [isExpanded, setIsExpanded] = useState(isStreaming);
  const wasStreamingRef = useRef(isStreaming);

  // Auto-expand when streaming starts (but allow user to collapse while streaming),
  // and auto-collapse when streaming ends.
  useEffect(() => {
    if (!wasStreamingRef.current && isStreaming) {
      // Streaming just started - expand by default
      setIsExpanded(true);
    }
    if (wasStreamingRef.current && !isStreaming) {
      // Streaming just ended - collapse the accordion
      setIsExpanded(false);
    }
    wasStreamingRef.current = isStreaming;
  }, [isStreaming]);

  // Split content into lines and get the first line as preview
  const lines = content.split("\n").filter((line) => line.trim() !== "");
  const firstLine = lines[0] || content;
  const hasMultipleLines = lines.length > 1;
  const previewText =
    firstLine.length > 60 ? `${firstLine.substring(0, 60)}...` : firstLine;

  return (
    <div className="max-w-[85%] mb-3">
      {/* Thin collapsible header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          "w-full flex items-center gap-2 px-3 py-1.5 text-xs",
          "bg-blue-50 hover:bg-blue-100 border border-blue-200 rounded-md",
          "text-blue-700 transition-colors cursor-pointer",
          "dark:bg-blue-950/30 dark:hover:bg-blue-950/50 dark:border-blue-800/50 dark:text-blue-300"
        )}
      >
        <ChevronDownIcon
          className={cn(
            "h-3 w-3 transition-transform shrink-0",
            isExpanded ? "rotate-180" : "rotate-0"
          )}
        />
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="font-medium shrink-0">Thinking</span>
          {!isExpanded && (
            <span className="text-blue-600 dark:text-blue-400 truncate">
              {previewText}
            </span>
          )}
          {!isExpanded && hasMultipleLines && (
            <span className="text-blue-500 dark:text-blue-500 shrink-0 opacity-70">
              +{lines.length - 1} more
            </span>
          )}
        </div>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div
          className={cn(
            "mt-1 px-3 py-2 text-xs",
            "bg-blue-50/80 border border-blue-200 rounded-md",
            "text-blue-800 whitespace-pre-wrap",
            "dark:bg-blue-950/20 dark:border-blue-800/30 dark:text-blue-200"
          )}
        >
          {content}
        </div>
      )}
    </div>
  );
}
