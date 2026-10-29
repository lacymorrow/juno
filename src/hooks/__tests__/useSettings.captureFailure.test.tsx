import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const handlers = new Map<string, (payload: unknown) => void>();
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

import { invoke } from "@tauri-apps/api/core";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useSettings } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  handlers.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) =>
    Promise.resolve(command === COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_STATUS ? true : undefined),
  );
});

async function listeningHook() {
  const hook = renderHook(() => useSettings());
  await waitFor(() => expect(hook.result.current.alwaysListeningActive).toBe(true));
  return hook;
}

function fail(source?: "recording" | "wake") {
  const handler = handlers.get(EVENTS.VOICE_CAPTURE_FAILED);
  expect(handler).toBeDefined();
  act(() => {
    handler?.({
      code: "stream_start_failed",
      message: "Shure MV7 would not start recording.",
      device: "Shure MV7",
      listening: false,
      ...(source ? { source } : {}),
    });
  });
}

describe("useSettings: a capture failure turns off only the side that failed", () => {
  it("leaves the wake phrase switch on when a recording's microphone failed", async () => {
    const hook = await listeningHook();
    fail("recording");
    expect(hook.result.current.captureFailure?.code).toBe("stream_start_failed");
    expect(hook.result.current.alwaysListeningActive).toBe(true);
  });

  it("turns the switch off when the wake phrase's microphone failed", async () => {
    const hook = await listeningHook();
    fail("wake");
    expect(hook.result.current.alwaysListeningActive).toBe(false);
  });

  it("turns the switch off when the failure does not say which", async () => {
    const hook = await listeningHook();
    fail();
    expect(hook.result.current.alwaysListeningActive).toBe(false);
  });
});
