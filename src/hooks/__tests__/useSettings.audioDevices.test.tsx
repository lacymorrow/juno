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
import { useSettings, type AudioDeviceChoices } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

/** The lists as Rust reports them, with `inputs` in the order given. */
function connected(...inputs: string[]): AudioDeviceChoices {
  return {
    inputs: inputs.map((id) => ({ id, name: id, is_default: id === inputs[0] })),
    outputs: [],
    chosen_input: null,
    chosen_output: null,
    effective_input: inputs[0] ?? null,
    missing_input: null,
    missing_output: null,
  };
}

function listed(choices: AudioDeviceChoices | null) {
  return choices?.inputs.map((device) => device.id);
}

/** Rust answers every device read with `choices`. */
function answers(choices: AudioDeviceChoices) {
  invokeMock.mockImplementation(((command: string) =>
    command === COMMANDS.AUDIO_LIST_AUDIO_DEVICES
      ? Promise.resolve(choices)
      : Promise.resolve(undefined)) as typeof invoke);
}

beforeEach(() => {
  handlers.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation(() => Promise.resolve(undefined));
});

// The defect: the list was read once, when the pane opened. CoreAudio knew a
// device had come or gone and nothing said so, so the only way to see the real
// set of devices was to close Settings and open it again.
describe("useSettings: the device list follows the hardware", () => {
  it("adds a microphone the moment it is plugged in", async () => {
    answers(connected("built-in"));
    const { result } = renderHook(() => useSettings());
    await act(async () => {
      await result.current.loadAudioDevices();
    });
    expect(listed(result.current.audioDevices)).toEqual(["built-in"]);

    await act(async () => {
      handlers.get(EVENTS.AUDIO_DEVICES_CHANGED)?.(connected("built-in", "usb-mic"));
    });

    expect(listed(result.current.audioDevices)).toEqual(["built-in", "usb-mic"]);
  });

  it("drops a microphone the moment it is unplugged", async () => {
    answers(connected("built-in", "usb-mic"));
    const { result } = renderHook(() => useSettings());
    await act(async () => {
      await result.current.loadAudioDevices();
    });
    expect(listed(result.current.audioDevices)).toEqual(["built-in", "usb-mic"]);

    await act(async () => {
      handlers.get(EVENTS.AUDIO_DEVICES_CHANGED)?.(connected("built-in"));
    });

    expect(listed(result.current.audioDevices)).toEqual(["built-in"]);
  });

  it("keeps the selection the change was reported with", async () => {
    const { result } = renderHook(() => useSettings());
    const afterUnplug: AudioDeviceChoices = {
      ...connected("built-in"),
      chosen_input: "uid-usb-mic",
      effective_input: "built-in",
      missing_input: { id: "uid-usb-mic", name: "USB Microphone", is_default: false },
    };

    await act(async () => {
      handlers.get(EVENTS.AUDIO_DEVICES_CHANGED)?.(afterUnplug);
    });

    // Resolved by UID by Rust, and drawn as Rust resolved it: the row holds
    // the choice and the pane can say it is not connected.
    expect(result.current.audioDevices?.chosen_input).toBe("uid-usb-mic");
    expect(result.current.audioDevices?.missing_input?.name).toBe("USB Microphone");
  });

  // The race the voice list already learned: an answer is only drawn if it is
  // still the newest one asked for.
  it("a read asked for before the unplug does not put the device back", async () => {
    const reads: Array<(choices: AudioDeviceChoices) => void> = [];
    invokeMock.mockImplementation(((command: string) =>
      command === COMMANDS.AUDIO_LIST_AUDIO_DEVICES
        ? new Promise<AudioDeviceChoices>((resolve) => reads.push(resolve))
        : Promise.resolve(undefined)) as typeof invoke);

    const { result } = renderHook(() => useSettings());
    let read: Promise<void> = Promise.resolve();
    act(() => {
      read = result.current.loadAudioDevices();
    });
    await waitFor(() => expect(reads.length).toBeGreaterThan(0));

    // The mic is unplugged while that read is still in flight.
    await act(async () => {
      handlers.get(EVENTS.AUDIO_DEVICES_CHANGED)?.(connected("built-in"));
    });
    expect(listed(result.current.audioDevices)).toEqual(["built-in"]);

    // It answers with the pair that was connected when it was asked.
    await act(async () => {
      reads.shift()?.(connected("built-in", "usb-mic"));
      await read;
    });

    expect(listed(result.current.audioDevices)).toEqual(["built-in"]);
  });
});
