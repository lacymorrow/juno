/**
 * LAC-7987. The engine Settings paints before Rust has answered must be the
 * generated default, not a copy of it.
 *
 * The default is platform-dependent (the Mac's own voice on macOS, Kokoro
 * elsewhere), so a hardcoded engine in the hook paints the wrong row on a
 * fresh install until the first read lands.
 *
 * The generated constant is stubbed to a sentinel on purpose. Asserting
 * against the real value would pass vacuously: `isMac` is false under jsdom,
 * so `SETTING_DEFAULTS.TTS_PROVIDER` evaluates to "kokoro" here, which is
 * exactly the string the bug hardcoded. The sentinel is the only form of this
 * test that fails against the old code.
 */
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// `vi.hoisted`, because `vi.mock` factories are hoisted above ordinary
// top-level consts and a plain `const` here is a ReferenceError at mock time.
const { SENTINEL } = vi.hoisted(() => ({ SENTINEL: "sentinel-engine" }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("@/hooks/useEventListener", () => ({ useEventListener: () => {} }));

vi.mock("@/lib/constants.generated", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/constants.generated")>();
  return {
    ...actual,
    SETTING_DEFAULTS: { ...actual.SETTING_DEFAULTS, TTS_PROVIDER: SENTINEL },
  };
});

import { invoke } from "@tauri-apps/api/core";
import { COMMANDS, SETTING_DEFAULTS } from "@/lib/constants.generated";
import { useSettings } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("useSettings: the engine shown before Rust answers", () => {
  it("is the generated default, not a hardcoded engine", async () => {
    // Every read is left unanswered, so the hook is held in the state a fresh
    // install is in for the first frames: nothing from Rust yet.
    const pending = new Promise<never>(() => {});
    invokeMock.mockImplementation(((command: string) => {
      if (command === COMMANDS.AUDIO_GET_JUNO_VOICES) return pending;
      return pending;
    }) as typeof invoke);

    const { result } = renderHook(() => useSettings());

    expect(SETTING_DEFAULTS.TTS_PROVIDER).toBe(SENTINEL);
    expect(result.current.ttsProvider).toBe(SENTINEL);

    // Still the default after the hook's mount effects have run, so this is
    // the value the pane actually paints rather than a pre-effect artifact.
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(result.current.ttsProvider).toBe(SENTINEL);
  });
});
