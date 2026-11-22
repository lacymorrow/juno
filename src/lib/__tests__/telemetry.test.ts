import { beforeEach, describe, expect, it, vi } from "vitest";

const values = new Map<string, unknown>();
vi.mock("@tauri-apps/plugin-store", () => ({
  load: async () => ({
    get: async (k: string) => values.get(k),
    set: async (k: string, v: unknown) => void values.set(k, v),
    save: async () => {},
  }),
}));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "1.2.3" }));

import { isTelemetryEnabled, setTelemetryEnabled, trackAppOpen, trackError, trackFeature } from "../telemetry";

describe("telemetry", () => {
  const fetchMock = vi.fn(async (_url?: string, _init?: RequestInit) => new Response(null, { status: 200 }));
  beforeEach(() => {
    values.clear();
    fetchMock.mockClear();
    vi.stubGlobal("fetch", fetchMock);
  });

  it("is on by default and sends app_open with the app key", async () => {
    expect(await isTelemetryEnabled()).toBe(true);
    await trackAppOpen();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect((init.headers as Record<string, string>)["App-Key"]).toBe("A-US-6551635026");
    expect(JSON.parse(init.body as string).eventName).toBe("app_open");
  });

  it("sends nothing once turned off, and remembers it", async () => {
    await setTelemetryEnabled(false);
    await trackFeature("dictation");
    await trackError("TypeError");
    expect(fetchMock).not.toHaveBeenCalled();
    expect(await isTelemetryEnabled()).toBe(false);
  });

  it("carries only the feature name and error class as props", async () => {
    await trackFeature("dictation");
    await trackError("TypeError");
    const bodies = fetchMock.mock.calls.map((c) => JSON.parse((c[1] as RequestInit).body as string));
    expect(bodies[0].props).toEqual({ name: "dictation" });
    expect(bodies[1].props).toEqual({ class: "TypeError" });
  });

  it("never throws when the network fails", async () => {
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    await expect(trackAppOpen()).resolves.toBeUndefined();
  });
});
