import { act, renderHook } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "@/hooks/useSettings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const handlers = new Map<string, (payload: unknown) => void>();
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

/** A value `loadAllSettings` reads through the cache. */
const TTS_PROVIDER = "get_tts_provider_command";
/** A value only `loadToolConfigurations` reads through the cache. */
const TOOL_CONFIGS = "get_tool_configurations";

const invokeMock = vi.mocked(invoke);

const callsTo = (command: string) =>
  invokeMock.mock.calls.filter(([name]) => name === command).length;

describe("useSettings: a reset is visible", () => {
  beforeEach(() => {
    handlers.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) =>
      Promise.resolve(command === TOOL_CONFIGS ? {} : undefined),
    );
  });

  // "Reset all settings" writes the defaults and then calls loadAllSettings.
  // The cache inside the hook holds each value for 30 seconds, so that reload
  // was served the pre-reset values straight back out of it: every pane
  // redrew exactly what it already showed, while the toast said the reset had
  // worked. An explicit reload has to go back to Rust.
  it("re-reads from the backend rather than serving a cached value", async () => {
    const { result } = renderHook(() => useSettings());

    await act(async () => {
      await result.current.loadAllSettings();
    });
    const afterFirstLoad = callsTo(TTS_PROVIDER);
    expect(afterFirstLoad).toBeGreaterThan(0);

    // A second load, well inside the cache's 30-second window.
    await act(async () => {
      await result.current.loadAllSettings();
    });

    expect(callsTo(TTS_PROVIDER)).toBeGreaterThan(afterFirstLoad);
  });

  // The backend emits settings_changed on every whole-settings write, which is
  // what a reset is. Panes that read their own values rather than calling
  // loadAllSettings have to see the new ones too, so the event drops the whole
  // cache and not just the one key its handler goes on to read.
  it("drops every cached value when the backend reports a settings change", async () => {
    const { result } = renderHook(() => useSettings());

    await act(async () => {
      await result.current.loadToolConfigurations();
    });
    const afterFirstLoad = callsTo(TOOL_CONFIGS);
    expect(afterFirstLoad).toBeGreaterThan(0);

    // Without the event invalidating, this second read is served the stale
    // configurations and the Tools pane keeps drawing the pre-reset state.
    act(() => {
      handlers.get("settings_changed")?.({ agent: { execution_mode: "single" } });
    });
    await act(async () => {
      await result.current.loadToolConfigurations();
    });

    expect(callsTo(TOOL_CONFIGS)).toBeGreaterThan(afterFirstLoad);
  });
});
