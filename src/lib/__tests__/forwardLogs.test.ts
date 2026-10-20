import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FLUSH_MS, MAX_PER_FLUSH, formatArg, installForwardLogs, type LogEntry } from "@/lib/forwardLogs";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));

describe("forwardLogs", () => {
  let sent: LogEntry[][];
  let undo: () => void;
  const send = vi.fn((entries: LogEntry[]) => {
    sent.push(entries);
    return Promise.resolve();
  });

  beforeEach(() => {
    vi.useFakeTimers();
    sent = [];
    send.mockClear();
    // Keep the test output quiet; the originals are what the wrapper calls.
    for (const level of ["error", "warn", "info", "log", "debug"] as const) {
      vi.spyOn(console, level).mockImplementation(() => {});
    }
    undo = installForwardLogs(send, true);
  });
  afterEach(() => {
    undo();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("sends a tick's console lines to Rust together, with their levels", () => {
    console.info("[Island] opened", { face: "chat" });
    console.warn("careful");
    expect(send).not.toHaveBeenCalled();
    vi.advanceTimersByTime(FLUSH_MS);
    expect(sent).toEqual([
      [
        { level: "info", message: '[Island] opened {"face":"chat"}' },
        { level: "warn", message: "careful" },
      ],
    ]);
  });

  it("forwards uncaught errors and unhandled rejections", () => {
    window.dispatchEvent(new ErrorEvent("error", { message: "boom", filename: "bar.js", lineno: 3, colno: 7 }));
    const rejection = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(rejection, "reason", { value: new Error("nope") });
    window.dispatchEvent(rejection);
    vi.advanceTimersByTime(FLUSH_MS);
    const [batch] = sent;
    expect(batch[0]).toEqual({ level: "error", message: "Uncaught boom (bar.js:3:7)" });
    expect(batch[1].level).toBe("error");
    expect(batch[1].message).toContain("Unhandled rejection: Error: nope");
  });

  it("caps a runaway page and says how much it dropped", () => {
    for (let i = 0; i < MAX_PER_FLUSH + 5; i++) console.log(`line ${i}`);
    vi.advanceTimersByTime(FLUSH_MS);
    const [batch] = sent;
    expect(batch).toHaveLength(MAX_PER_FLUSH + 1);
    expect(batch[MAX_PER_FLUSH].message).toContain("5 more console lines dropped");
  });

  it("a failed send is not logged back into the queue", async () => {
    send.mockImplementationOnce(() => Promise.reject(new Error("ipc down")));
    console.error("first");
    vi.advanceTimersByTime(FLUSH_MS);
    await Promise.resolve();
    vi.advanceTimersByTime(FLUSH_MS);
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("flushes what is left when the page goes away", () => {
    console.log("last words");
    window.dispatchEvent(new Event("pagehide"));
    expect(sent).toEqual([[{ level: "log", message: "last words" }]]);
  });
});

describe("formatArg", () => {
  it("keeps strings, names errors, survives circular objects", () => {
    expect(formatArg("plain")).toBe("plain");
    expect(formatArg(new TypeError("bad"))).toMatch(/^TypeError: bad/);
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(formatArg(loop)).toBe("[object Object]");
    expect(formatArg(undefined)).toBe("undefined");
  });
});
