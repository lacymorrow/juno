import { cn } from "@/lib/utils";
import {
  CheckCircleIcon,
  ChevronDownIcon,
  PlayIcon,
  XCircleIcon,
} from "lucide-react";
import { useState } from "react";

interface ToolCallRequestProps {
  toolName: string;
  toolArgs?: any;
  content?: string;
  timestamp?: number;
}

interface ToolCallResultProps {
  toolName: string;
  toolOutput?: any;
  success: boolean;
  content?: string;
  screenshot_base64?: string;
  timestamp?: number;
}

export function ToolCallRequest({
  toolName,
  toolArgs,
  content,
}: ToolCallRequestProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hasArgs = toolArgs && Object.keys(toolArgs).length > 0;

  // Format computer tool names for better readability
  const formatToolName = (name: string) => {
    // Handle computer use tools with descriptive names
    if (name.startsWith("computer/")) {
      return name;
    }
    return name;
  };

  // Extract action description from computer tool names
  const getActionDescription = (name: string) => {
    if (name.startsWith("computer/")) {
      const action = name.replace("computer/", "");

      // Handle specific patterns
      if (action.startsWith("click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords ? `Click at (${coords[1]}, ${coords[2]})` : "Click";
      }
      if (action.startsWith("right_click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords
          ? `Right-click at (${coords[1]}, ${coords[2]})`
          : "Right-click";
      }
      if (action.startsWith("double_click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords
          ? `Double-click at (${coords[1]}, ${coords[2]})`
          : "Double-click";
      }
      if (action.startsWith("type(")) {
        const text = action.match(/type\("(.+?)"\)/);
        return text ? `Type "${text[1]}"` : "Type text";
      }
      if (action.startsWith("press_key(")) {
        const key = action.match(/press_key\((.+?)\)/);
        return key ? `Press ${key[1]}` : "Press key";
      }
      if (action.startsWith("scroll_")) {
        const match = action.match(
          /scroll_(\w+)\((\d+),\s*(\d+)\s*×\s*(\d+)\)/
        );
        return match
          ? `Scroll ${match[1]} ${match[4]}x at (${match[2]}, ${match[3]})`
          : action.replace(/_/g, " ");
      }
      if (action.startsWith("drag(")) {
        const coords = action.match(/drag\((\d+),(\d+)\s*→\s*(\d+),(\d+)\)/);
        return coords
          ? `Drag from (${coords[1]}, ${coords[2]}) to (${coords[3]}, ${coords[4]})`
          : "Drag";
      }
      if (action === "screenshot") {
        return "Take screenshot";
      }
      if (action === "get_cursor_position") {
        return "Get cursor position";
      }

      // Default: capitalize and replace underscores
      return action.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
    }

    return null;
  };

  const displayName = formatToolName(toolName);
  const actionDescription = getActionDescription(toolName);

  return (
    <div className="max-w-[85%] mb-3">
      {/* Tool call header */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 text-sm",
          "bg-purple-50 border border-purple-200 rounded-lg",
          "text-purple-800",
          "dark:bg-purple-950/30 dark:border-purple-800/50 dark:text-purple-300"
        )}
      >
        <PlayIcon className="h-4 w-4 text-purple-600 dark:text-purple-400 shrink-0" />
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="font-medium shrink-0">Tool Call:</span>
          <code className="font-mono bg-purple-100 dark:bg-purple-900/50 px-2 py-0.5 rounded text-sm">
            {displayName}
          </code>
          {actionDescription && (
            <span className="text-purple-600 dark:text-purple-400 text-xs">
              {actionDescription}
            </span>
          )}
        </div>
        {hasArgs && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-purple-600 dark:text-purple-400 hover:text-purple-700 dark:hover:text-purple-300"
          >
            <ChevronDownIcon
              className={cn(
                "h-4 w-4 transition-transform",
                isExpanded ? "rotate-180" : "rotate-0"
              )}
            />
          </button>
        )}
      </div>

      {/* Content */}
      {content && (
        <div className="mt-1 px-3 py-2 text-sm bg-purple-50/50 dark:bg-purple-950/20 border border-purple-200/50 dark:border-purple-800/30 rounded-lg">
          {content}
        </div>
      )}

      {/* Expanded arguments */}
      {isExpanded && hasArgs && (
        <div className="mt-1 px-3 py-2 bg-purple-50/70 dark:bg-purple-950/20 border border-purple-200 dark:border-purple-800/30 rounded-lg">
          <div className="text-xs font-medium text-purple-700 dark:text-purple-300 mb-1">
            Arguments:
          </div>
          <pre className="text-xs text-purple-800 dark:text-purple-200 overflow-x-auto whitespace-pre-wrap">
            {JSON.stringify(toolArgs, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}

export function ToolCallResult({
  toolName,
  toolOutput,
  success,
  content,
  screenshot_base64,
}: ToolCallResultProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hasOutput = toolOutput !== undefined && toolOutput !== null;

  // Format computer tool names for better readability (reuse from ToolCallRequest)
  const formatToolName = (name: string) => {
    // Handle computer use tools with descriptive names
    if (name.startsWith("computer/")) {
      return name;
    }
    return name;
  };

  // Extract action description from computer tool names (reuse from ToolCallRequest)
  const getActionDescription = (name: string) => {
    if (name.startsWith("computer/")) {
      const action = name.replace("computer/", "");

      // Handle specific patterns
      if (action.startsWith("click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords ? `Click at (${coords[1]}, ${coords[2]})` : "Click";
      }
      if (action.startsWith("right_click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords
          ? `Right-click at (${coords[1]}, ${coords[2]})`
          : "Right-click";
      }
      if (action.startsWith("double_click(")) {
        const coords = action.match(/\((\d+),\s*(\d+)\)/);
        return coords
          ? `Double-click at (${coords[1]}, ${coords[2]})`
          : "Double-click";
      }
      if (action.startsWith("type(")) {
        const text = action.match(/type\("(.+?)"\)/);
        return text ? `Type "${text[1]}"` : "Type text";
      }
      if (action.startsWith("press_key(")) {
        const key = action.match(/press_key\((.+?)\)/);
        return key ? `Press ${key[1]}` : "Press key";
      }
      if (action.startsWith("scroll_")) {
        const match = action.match(
          /scroll_(\w+)\((\d+),\s*(\d+)\s*×\s*(\d+)\)/
        );
        return match
          ? `Scroll ${match[1]} ${match[4]}x at (${match[2]}, ${match[3]})`
          : action.replace(/_/g, " ");
      }
      if (action.startsWith("drag(")) {
        const coords = action.match(/drag\((\d+),(\d+)\s*→\s*(\d+),(\d+)\)/);
        return coords
          ? `Drag from (${coords[1]}, ${coords[2]}) to (${coords[3]}, ${coords[4]})`
          : "Drag";
      }
      if (action === "screenshot") {
        return "Take screenshot";
      }
      if (action === "get_cursor_position") {
        return "Get cursor position";
      }

      // Default: capitalize and replace underscores
      return action.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
    }

    return null;
  };

  const displayName = formatToolName(toolName);
  const actionDescription = getActionDescription(toolName);

  return (
    <div className="max-w-[85%] mb-3">
      {/* Tool result header */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 text-sm",
          success
            ? "bg-green-50 border border-green-200 text-green-800 dark:bg-green-950/30 dark:border-green-800/50 dark:text-green-300"
            : "bg-red-50 border border-red-200 text-red-800 dark:bg-red-950/30 dark:border-red-800/50 dark:text-red-300",
          "rounded-lg"
        )}
      >
        {success ? (
          <CheckCircleIcon className="h-4 w-4 text-green-600 dark:text-green-400 shrink-0" />
        ) : (
          <XCircleIcon className="h-4 w-4 text-red-600 dark:text-red-400 shrink-0" />
        )}
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="font-medium shrink-0">
            Tool {success ? "Success" : "Failed"}:
          </span>
          <code
            className={cn(
              "font-mono px-2 py-0.5 rounded text-sm",
              success
                ? "bg-green-100 dark:bg-green-900/50"
                : "bg-red-100 dark:bg-red-900/50"
            )}
          >
            {displayName}
          </code>
          {actionDescription && (
            <span
              className={cn(
                "text-xs",
                success
                  ? "text-green-600 dark:text-green-400"
                  : "text-red-600 dark:text-red-400"
              )}
            >
              {actionDescription}
            </span>
          )}
        </div>
        {hasOutput && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className={cn(
              "hover:opacity-70",
              success
                ? "text-green-600 dark:text-green-400"
                : "text-red-600 dark:text-red-400"
            )}
          >
            <ChevronDownIcon
              className={cn(
                "h-4 w-4 transition-transform",
                isExpanded ? "rotate-180" : "rotate-0"
              )}
            />
          </button>
        )}
      </div>

      {/* Content */}
      {content && (
        <div
          className={cn(
            "mt-1 px-3 py-2 text-sm border rounded-lg",
            success
              ? "bg-green-50/50 border-green-200/50 dark:bg-green-950/20 dark:border-green-800/30"
              : "bg-red-50/50 border-red-200/50 dark:bg-red-950/20 dark:border-red-800/30"
          )}
        >
          {content}
        </div>
      )}

      {/* Expanded output */}
      {isExpanded && hasOutput && (
        <div
          className={cn(
            "mt-1 px-3 py-2 border rounded-lg",
            success
              ? "bg-green-50/70 border-green-200 dark:bg-green-950/20 dark:border-green-800/30"
              : "bg-red-50/70 border-red-200 dark:bg-red-950/20 dark:border-red-800/30"
          )}
        >
          <div
            className={cn(
              "text-xs font-medium mb-1",
              success
                ? "text-green-700 dark:text-green-300"
                : "text-red-700 dark:text-red-300"
            )}
          >
            Output:
          </div>
          <pre
            className={cn(
              "text-xs overflow-x-auto whitespace-pre-wrap",
              success
                ? "text-green-800 dark:text-green-200"
                : "text-red-800 dark:text-red-200"
            )}
          >
            {typeof toolOutput === "string"
              ? toolOutput
              : JSON.stringify(toolOutput, null, 2)}
          </pre>
        </div>
      )}

      {/* Screenshot */}
      {screenshot_base64 && (
        <div
          className={cn(
            "mt-2 border-t pt-2",
            success ? "border-green-200/50" : "border-red-200/50"
          )}
        >
          <div className="text-xs text-muted-foreground mb-1">
            Screenshot captured by tool:
          </div>
          <div className="relative">
            <img
              src={`data:image/png;base64,${screenshot_base64}`}
              alt="Tool Screenshot"
              className="rounded w-full object-contain max-h-[300px] border border-border shadow-sm"
            />
            <div className="absolute inset-0 bg-gradient-to-t from-background/20 to-transparent pointer-events-none"></div>
          </div>
        </div>
      )}
    </div>
  );
}
