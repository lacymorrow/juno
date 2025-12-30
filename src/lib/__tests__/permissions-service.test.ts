import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  getPermissionsStatus,
  __resetPermissionsServiceCacheForTests,
} from "../permissions-service";
import type {
  PermissionsState,
  AppPermissionStatus,
} from "@/types/settings";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn(() => Promise.resolve(() => {})),
  })),
}));

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2025-01-01T00:00:00.000Z"));
  __resetPermissionsServiceCacheForTests();
  mockInvoke.mockReset();
});

afterEach(() => {
  __resetPermissionsServiceCacheForTests();
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe("permissions-service", () => {
  it("returns cached permissions within TTL window", async () => {
    const initial = buildPermissionsState({ app_name: "Initial" });
    mockInvoke.mockResolvedValueOnce(initial);

    const firstResult = await getPermissionsStatus();
    expect(firstResult).toEqual(initial);
    expect(mockInvoke).toHaveBeenCalledTimes(1);

    const secondResult = await getPermissionsStatus();
    expect(secondResult).toBe(initial);
    expect(mockInvoke).toHaveBeenCalledTimes(1);

    const sixSecondsLater = new Date(Date.now() + 6000);
    vi.setSystemTime(sixSecondsLater);

    const refreshed = buildPermissionsState({ app_name: "Refreshed" });
    mockInvoke.mockResolvedValueOnce(refreshed);

    const thirdResult = await getPermissionsStatus();
    expect(thirdResult).toEqual(refreshed);
    expect(mockInvoke).toHaveBeenCalledTimes(2);
  });

  it("forceRefresh waits for pending request then starts fresh", async () => {
    const deferred = createDeferred<PermissionsState>();
    mockInvoke.mockImplementationOnce(() => deferred.promise);

    const pendingCall = getPermissionsStatus();

    const fresh = buildPermissionsState({ app_name: "Fresh" });
    mockInvoke.mockResolvedValueOnce(fresh);
    const forcedCall = getPermissionsStatus(true);

    // forceRefresh waits for pending request - only 1 call so far
    expect(mockInvoke).toHaveBeenCalledTimes(1);

    const stale = buildPermissionsState({ app_name: "Stale" });
    deferred.resolve(stale);

    // Both calls complete
    const pendingResult = await pendingCall;
    const forcedResult = await forcedCall;

    // Pending call gets its original result
    expect(pendingResult.app_name).toBe("Stale");
    // forceRefresh gets fresh data after waiting
    expect(forcedResult.app_name).toBe("Fresh");
    // Now 2 calls total (pending completed, then fresh request started)
    expect(mockInvoke).toHaveBeenCalledTimes(2);

    const cachedResult = await getPermissionsStatus();
    expect(cachedResult.app_name).toBe("Fresh");
    expect(mockInvoke).toHaveBeenCalledTimes(2);
  });

  it("clears pending request on error", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("Network error"));

    await expect(getPermissionsStatus()).rejects.toThrow("Network error");

    // Next call should start a fresh request
    const fresh = buildPermissionsState({ app_name: "Fresh" });
    mockInvoke.mockResolvedValueOnce(fresh);

    const result = await getPermissionsStatus();
    expect(result.app_name).toBe("Fresh");
    expect(mockInvoke).toHaveBeenCalledTimes(2);
  });
});

function buildPermissionsState(
  overrides: Partial<PermissionsState> = {}
): PermissionsState {
  const makePermission = (
    permission_type: string,
    granted = false,
    required = false
  ): AppPermissionStatus => ({
    permission_type,
    granted,
    required,
    description: `${permission_type} description`,
    instructions: `${permission_type} instructions`,
  });

  const base: PermissionsState = {
    accessibility: makePermission("accessibility", true, true),
    screen_recording: makePermission("screen_recording", true, true),
    microphone: makePermission("microphone"),
    input_monitoring: makePermission("input_monitoring"),
    all_granted: true,
    app_name: "Juno",
  };

  return { ...base, ...overrides };
}

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

