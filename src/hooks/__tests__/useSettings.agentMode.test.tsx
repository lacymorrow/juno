import { act, renderHook } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useSettings } from "@/hooks/useSettings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Capture every handler the hook registers, keyed by event name, so a test
// can fire a backend event directly.
const handlers = new Map<string, (payload: unknown) => void>();
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

describe("useSettings: agent mode follows the backend", () => {
  beforeEach(() => {
    handlers.clear();
  });

  // Settings and the chat pane are separate windows, each with its own copy
  // of this hook. The one that did not make the change learns about it only
  // through the backend's settings_changed broadcast.
  it("updates agentMode when another window changes it", () => {
    const { result } = renderHook(() => useSettings());
    expect(result.current.agentMode).toBe("multi");

    act(() => {
      handlers.get("settings_changed")?.({ agent: { execution_mode: "single" } });
    });

    expect(result.current.agentMode).toBe("single");
  });

  it("ignores a settings_changed payload without an agent mode", () => {
    const { result } = renderHook(() => useSettings());

    act(() => {
      handlers.get("settings_changed")?.({});
    });

    expect(result.current.agentMode).toBe("multi");
  });
});
