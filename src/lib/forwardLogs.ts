import { invoke } from "@tauri-apps/api/core";

import { COMMANDS } from "@/lib/constants.generated";

/**
 * Every window's console, in Juno's log.
 *
 * `console.*` calls and uncaught errors are still printed where they were,
 * and are also sent to Rust (`frontend_log`), which writes them into
 * ~/Library/Logs/Juno as `[Web:<window>] …` lines between Rust's own. A page
 * that decided something on its own used to leave no trace there, so the log
 * could not show the front end and the back end disagreeing.
 *
 * Lines are batched per tick so a chatty page costs one IPC call, not one per
 * line, and a runaway page is capped rather than flooding the log.
 */

type Level = "error" | "warn" | "info" | "log" | "debug";
export interface LogEntry {
  level: Level;
  message: string;
}
type Send = (entries: LogEntry[]) => Promise<unknown>;

const LEVELS: Level[] = ["error", "warn", "info", "log", "debug"];
/** How long lines gather before they go to Rust together. */
export const FLUSH_MS = 200;
/** Lines kept per flush; past it a page is looping and one note says so. */
export const MAX_PER_FLUSH = 200;
/** Characters kept per argument (Rust caps the whole line too). */
const MAX_ARG_CHARS = 2000;

/** One console argument as text: strings as they are, errors with their stack. */
export function formatArg(arg: unknown): string {
  let text: string;
  if (typeof arg === "string") text = arg;
  else if (arg instanceof Error) text = arg.stack ? `${arg.name}: ${arg.message} ${arg.stack}` : `${arg.name}: ${arg.message}`;
  else if (arg === undefined) text = "undefined";
  else if (typeof arg === "function") text = `[function ${arg.name || "anonymous"}]`;
  else {
    try {
      text = JSON.stringify(arg) ?? String(arg);
    } catch {
      text = String(arg);
    }
  }
  return text.length > MAX_ARG_CHARS ? `${text.slice(0, MAX_ARG_CHARS)}…` : text;
}

export function formatArgs(args: unknown[]): string {
  return args.map(formatArg).join(" ");
}

/** Whether this page should forward: a real Tauri window, not a preview frame. */
function shouldForward(): boolean {
  if (typeof window === "undefined") return false;
  if (window.self !== window.top) return false;
  return "__TAURI_INTERNALS__" in window;
}

const defaultSend: Send = (entries) => invoke(COMMANDS.APP_FRONTEND_LOG, { entries });

/**
 * Start forwarding. Returns the undo, for tests. Installing twice is a no-op
 * returning the first undo, so a hot reload cannot double every line.
 */
let installed: (() => void) | null = null;
export function installForwardLogs(send: Send = defaultSend, force = false): () => void {
  if (installed) return installed;
  if (!force && !shouldForward()) return () => {};

  let queue: LogEntry[] = [];
  let dropped = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const flush = () => {
    timer = null;
    if (queue.length === 0 && dropped === 0) return;
    const batch = queue;
    if (dropped > 0) batch.push({ level: "warn", message: `${dropped} more console lines dropped (over ${MAX_PER_FLUSH} in ${FLUSH_MS} ms)` });
    queue = [];
    dropped = 0;
    // Never logged on failure: that would feed the queue it just emptied.
    send(batch).catch(() => {});
  };
  const enqueue = (level: Level, message: string) => {
    if (queue.length >= MAX_PER_FLUSH) dropped += 1;
    else queue.push({ level, message });
    if (timer === null) timer = setTimeout(flush, FLUSH_MS);
  };

  const originals = new Map<Level, (...args: unknown[]) => void>();
  for (const level of LEVELS) {
    const original = console[level].bind(console);
    originals.set(level, original);
    console[level] = (...args: unknown[]) => {
      original(...args);
      enqueue(level, formatArgs(args));
    };
  }

  const onError = (event: ErrorEvent) => {
    const where = event.filename ? ` (${event.filename}:${event.lineno}:${event.colno})` : "";
    const stack = event.error instanceof Error && event.error.stack ? ` ${event.error.stack}` : "";
    enqueue("error", `Uncaught ${event.message}${where}${stack}`);
  };
  const onRejection = (event: PromiseRejectionEvent) => {
    enqueue("error", `Unhandled rejection: ${formatArg(event.reason)}`);
  };
  // A window closing must not lose its last lines.
  const onHide = () => flush();
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  window.addEventListener("pagehide", onHide);

  installed = () => {
    for (const [level, original] of originals) console[level] = original;
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
    window.removeEventListener("pagehide", onHide);
    if (timer !== null) clearTimeout(timer);
    flush();
    installed = null;
  };
  return installed;
}
