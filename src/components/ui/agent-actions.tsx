/**
 * Interactive components for agent JSX responses.
 *
 * These components allow agent-rendered UI to trigger actions:
 * - ActionButton: Invoke a whitelisted Tauri command
 * - QueryButton: Submit a new query to the agent
 * - OpenButton: Open a URL or file path
 * - CopyButton: Copy text to clipboard
 *
 * Security: Only whitelisted commands can be invoked.
 * Components are registered in availableComponents for react-jsx-parser.
 */

import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useState, useCallback } from "react";
import {
  ExternalLink,
  Play,
  Copy,
  Check,
  Loader2,
  MessageSquare,
  FolderOpen,
  Terminal,
} from "lucide-react";

// ============================================================
// Security: Whitelisted Tauri commands
// ============================================================

/**
 * Commands that agent-rendered components are allowed to invoke.
 * This is the ONLY security boundary — react-jsx-parser cannot call
 * arbitrary functions, so we control what's available here.
 */
const ALLOWED_COMMANDS = new Set([
  // Open URLs and applications
  "open_url",
  "open_application",

  // System queries (read-only)
  "get_system_info",
  "capture_screenshot",

  // Agent interaction
  "submit_query",

  // UI state
  "ui_handle_interaction",
]);

function isCommandAllowed(command: string): boolean {
  return ALLOWED_COMMANDS.has(command);
}

// ============================================================
// ActionButton — invoke a whitelisted Tauri command
// ============================================================

interface ActionButtonProps {
  /** The Tauri command to invoke */
  command: string;
  /** Arguments to pass to the command (JSON-serializable) */
  args?: Record<string, unknown>;
  /** Button label */
  label?: string;
  /** Visual variant */
  variant?: "default" | "outline" | "ghost" | "destructive";
  /** Size */
  size?: "sm" | "default" | "lg";
  /** Optional icon name */
  icon?: string;
  /** Additional CSS classes */
  className?: string;
}

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  play: Play,
  terminal: Terminal,
  folder: FolderOpen,
  link: ExternalLink,
  copy: Copy,
  message: MessageSquare,
};

export function ActionButton({
  command,
  args,
  label = "Run",
  variant = "outline",
  size = "sm",
  icon,
  className,
}: ActionButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleClick = useCallback(async () => {
    if (!isCommandAllowed(command)) {
      setStatus("error");
      setErrorMsg(`Command not allowed: ${command}`);
      return;
    }

    setStatus("loading");
    setErrorMsg(null);

    try {
      await invoke(command, args || {});
      setStatus("success");
      setTimeout(() => setStatus("idle"), 2000);
    } catch (err) {
      setStatus("error");
      setErrorMsg(String(err));
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [command, args]);

  const Icon = icon ? ICON_MAP[icon] : null;

  const variantClasses = {
    default: "bg-primary text-primary-foreground hover:bg-primary/90",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
    ghost: "hover:bg-accent hover:text-accent-foreground",
    destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
  };

  const sizeClasses = {
    sm: "h-7 px-2.5 text-xs",
    default: "h-8 px-3 text-sm",
    lg: "h-9 px-4 text-sm",
  };

  return (
    <div className="inline-flex flex-col items-start">
      <button
        onClick={handleClick}
        disabled={status === "loading"}
        className={cn(
          "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          "disabled:pointer-events-none disabled:opacity-50",
          variantClasses[variant],
          sizeClasses[size],
          className,
        )}
      >
        {status === "loading" ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : status === "success" ? (
          <Check className="h-3 w-3 text-green-500" />
        ) : Icon ? (
          <Icon className="h-3 w-3" />
        ) : null}
        {status === "success" ? "Done" : label}
      </button>
      {status === "error" && errorMsg && (
        <span className="text-[10px] text-destructive mt-0.5 max-w-[200px] truncate">
          {errorMsg}
        </span>
      )}
    </div>
  );
}

// ============================================================
// QueryButton — submit a new query to the agent
// ============================================================

interface QueryButtonProps {
  /** The query to submit */
  query: string;
  /** Button label */
  label?: string;
  /** Visual variant */
  variant?: "default" | "outline" | "ghost";
  /** Size */
  size?: "sm" | "default" | "lg";
  /** Additional CSS classes */
  className?: string;
}

export function QueryButton({
  query,
  label,
  variant = "outline",
  size = "sm",
  className,
}: QueryButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");

  const handleClick = useCallback(async () => {
    setStatus("loading");
    try {
      await invoke("submit_query", { query });
    } catch (err) {
      console.error("QueryButton: Failed to submit query:", err);
      setStatus("error");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [query]);

  const variantClasses = {
    default: "bg-primary text-primary-foreground hover:bg-primary/90",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
    ghost: "hover:bg-accent hover:text-accent-foreground",
  };

  const sizeClasses = {
    sm: "h-7 px-2.5 text-xs",
    default: "h-8 px-3 text-sm",
    lg: "h-9 px-4 text-sm",
  };

  return (
    <button
      onClick={handleClick}
      disabled={status === "loading"}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        "disabled:pointer-events-none disabled:opacity-50",
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
    >
      {status === "loading" ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <MessageSquare className="h-3 w-3" />
      )}
      {label || query}
    </button>
  );
}

// ============================================================
// OpenButton — open a URL or file path
// ============================================================

interface OpenButtonProps {
  /** URL to open in default browser */
  url?: string;
  /** File path to open — converted to file:// URL */
  path?: string;
  /** Application name to open (e.g. "Safari", "Finder") */
  app?: string;
  /** Button label */
  label?: string;
  /** Visual variant */
  variant?: "default" | "outline" | "ghost";
  /** Size */
  size?: "sm" | "default" | "lg";
  /** Additional CSS classes */
  className?: string;
}

export function OpenButton({
  url,
  path,
  app,
  label,
  variant = "outline",
  size = "sm",
  className,
}: OpenButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");

  const handleClick = useCallback(async () => {
    setStatus("loading");
    try {
      if (app) {
        await invoke("open_application", { appName: app });
      } else if (path) {
        // Convert file path to file:// URL for open_url
        const fileUrl = path.startsWith("file://") ? path : `file://${path.replace(/^~/, "")}`;
        await invoke("open_url", { url: fileUrl });
      } else if (url) {
        await invoke("open_url", { url });
      }
      setStatus("idle");
    } catch (err) {
      console.error("OpenButton: Failed to open:", err);
      setStatus("error");
      setTimeout(() => setStatus("idle"), 3000);
    }
  }, [url, path, app]);

  const target = app || path || url || "";
  const displayLabel = label || (app ? app : path ? path.split("/").pop() : target);

  const variantClasses = {
    default: "bg-primary text-primary-foreground hover:bg-primary/90",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
    ghost: "hover:bg-accent hover:text-accent-foreground",
  };

  const sizeClasses = {
    sm: "h-7 px-2.5 text-xs",
    default: "h-8 px-3 text-sm",
    lg: "h-9 px-4 text-sm",
  };

  return (
    <button
      onClick={handleClick}
      disabled={status === "loading"}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        "disabled:pointer-events-none disabled:opacity-50",
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
    >
      {status === "loading" ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : path ? (
        <FolderOpen className="h-3 w-3" />
      ) : (
        <ExternalLink className="h-3 w-3" />
      )}
      {displayLabel}
    </button>
  );
}

// ============================================================
// CopyButton — copy text to clipboard
// ============================================================

interface CopyButtonProps {
  /** Text to copy */
  text: string;
  /** Button label */
  label?: string;
  /** Visual variant */
  variant?: "default" | "outline" | "ghost";
  /** Size */
  size?: "sm" | "default" | "lg";
  /** Additional CSS classes */
  className?: string;
}

export function CopyButton({
  text,
  label = "Copy",
  variant = "outline",
  size = "sm",
  className,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleClick = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("CopyButton: Failed to copy:", err);
    }
  }, [text]);

  const variantClasses = {
    default: "bg-primary text-primary-foreground hover:bg-primary/90",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
    ghost: "hover:bg-accent hover:text-accent-foreground",
  };

  const sizeClasses = {
    sm: "h-7 px-2.5 text-xs",
    default: "h-8 px-3 text-sm",
    lg: "h-9 px-4 text-sm",
  };

  return (
    <button
      onClick={handleClick}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
    >
      {copied ? (
        <Check className="h-3 w-3 text-green-500" />
      ) : (
        <Copy className="h-3 w-3" />
      )}
      {copied ? "Copied!" : label}
    </button>
  );
}
