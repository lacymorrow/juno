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
import { toast } from "sonner";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useSettings, type JunoVoiceList } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

function macList(selected: string): JunoVoiceList {
  return {
    provider: "system",
    engine: "system",
    engine_label: "Your Mac",
    note: null,
    engines: [
      { id: "system", name: "Your Mac" },
      { id: "kokoro", name: "Kokoro" },
    ],
    options: ["Samantha", "Daniel"].map((id) => ({
      id,
      kind: "voice",
      name: id,
      descriptor: "English.",
      selected: id === selected,
      speaks: true,
    })),
  };
}

function kokoroList(): JunoVoiceList {
  return {
    ...macList(""),
    provider: "kokoro",
    engine: "kokoro",
    engine_label: "Kokoro",
    options: [
      {
        id: "af_heart",
        kind: "voice",
        name: "Heart",
        descriptor: "American English, female.",
        selected: true,
        speaks: true,
      },
    ],
  };
}

function selectedId(list: JunoVoiceList | null) {
  return list?.options.find((option) => option.selected)?.id;
}

beforeEach(() => {
  handlers.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation(() => Promise.resolve(undefined));
  vi.mocked(toast.success).mockClear();
});

describe("useSettings: Juno's voice is whatever Rust answered last", () => {
  // Rust is modelled as Rust is: one current answer, which a pick changes.
  // Reads are held so a test can decide when each one lands.
  function fakeRust() {
    let current = macList("Samantha");
    const reads: Array<(list: JunoVoiceList) => void> = [];
    invokeMock.mockImplementation(((command: string, args?: Record<string, unknown>) => {
      if (command === COMMANDS.AUDIO_GET_JUNO_VOICES) {
        return new Promise<JunoVoiceList>((resolve) => reads.push(resolve));
      }
      if (command === COMMANDS.AUDIO_SET_JUNO_VOICE) {
        current = macList(String(args?.id));
        return Promise.resolve(current);
      }
      if (command === COMMANDS.TTS_SET_TTS_PROVIDER) {
        current = args?.provider === "kokoro" ? kokoroList() : macList("Samantha");
        return Promise.resolve(current);
      }
      return Promise.resolve(undefined);
    }) as typeof invoke);
    return {
      reads,
      get current() {
        return current;
      },
      /** Answer every read still waiting with what Rust holds now. */
      async settle() {
        await act(async () => {
          while (reads.length) reads.shift()?.(current);
        });
      },
    };
  }

  it("a slow read answered after a pick never undoes the pick", async () => {
    const rust = fakeRust();
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(rust.reads.length).toBeGreaterThan(0));
    await rust.settle();
    expect(selectedId(result.current.junoVoices)).toBe("Samantha");

    // The pane asks for the list, and Rust answers it before the pick lands:
    // the answer says Samantha. It is delivered last.
    const stale = rust.current;
    let read: Promise<void> = Promise.resolve();
    act(() => {
      read = result.current.loadJunoVoices();
    });
    await act(async () => {
      await result.current.handleJunoVoiceChange("Daniel");
    });
    expect(selectedId(result.current.junoVoices)).toBe("Daniel");

    await act(async () => {
      rust.reads.shift()?.(stale);
      await read;
    });
    expect(selectedId(result.current.junoVoices)).toBe("Daniel");
  });

  it("changing the engine draws the engine and voices Rust answered with", async () => {
    const rust = fakeRust();
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(rust.reads.length).toBeGreaterThan(0));
    await rust.settle();

    await act(async () => {
      await result.current.handleTtsProviderChange("kokoro");
    });

    expect(invokeMock).toHaveBeenCalledWith(COMMANDS.TTS_SET_TTS_PROVIDER, { provider: "kokoro" });
    expect(result.current.ttsProvider).toBe("kokoro");
    expect(result.current.junoVoices?.engine).toBe("kokoro");
    expect(result.current.junoVoices?.options.map((o) => o.id)).toEqual(["af_heart"]);
    // The new list is the confirmation; a toast on top of it is noise.
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("reads the voices again when a local engine finishes loading", async () => {
    renderHook(() => useSettings());
    invokeMock.mockClear();

    await act(async () => {
      handlers.get(EVENTS.JUNO_VOICE_ENGINE_READY)?.("kokoro");
    });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.AUDIO_GET_JUNO_VOICES),
    );
  });
});

describe("useSettings: how fast Juno speaks", () => {
  it("sends the rate to Rust and draws the list Rust answers with", async () => {
    const answered: JunoVoiceList = {
      ...macList("Samantha"),
      speed: { value: 1.5, min: 0.75, max: 2 },
    };
    invokeMock.mockImplementation(((command: string) => {
      if (command === COMMANDS.AUDIO_SET_JUNO_VOICE_RATE) return Promise.resolve(answered);
      if (command === COMMANDS.AUDIO_GET_JUNO_VOICES) return Promise.resolve(macList("Samantha"));
      return Promise.resolve(undefined);
    }) as typeof invoke);

    const { result } = renderHook(() => useSettings());
    // The pane's own read lands first; the rate is a later answer.
    await waitFor(() => expect(result.current.junoVoices).not.toBeNull());
    expect(result.current.junoVoices?.speed).toBeUndefined();
    await act(async () => {
      await result.current.handleJunoVoiceRateChange(1.5);
    });

    expect(invokeMock).toHaveBeenCalledWith(COMMANDS.AUDIO_SET_JUNO_VOICE_RATE, {
      rate: 1.5,
    });
    expect(result.current.junoVoices?.speed).toEqual({ value: 1.5, min: 0.75, max: 2 });
  });

  it("says so when Rust refuses the rate", async () => {
    invokeMock.mockImplementation(((command: string) =>
      command === COMMANDS.AUDIO_SET_JUNO_VOICE_RATE
        ? Promise.reject("The speed has to be a number.")
        : Promise.resolve(undefined)) as typeof invoke);

    const { result } = renderHook(() => useSettings());
    await act(async () => {
      await result.current.handleJunoVoiceRateChange(Number.NaN);
    });

    expect(toast.error).toHaveBeenCalledWith("The speed has to be a number.");
  });
});
