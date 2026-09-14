import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { shouldAutoPromptMicrophone } from "@/components/onboarding/Onboarding";

function perm(granted: boolean, permission_type: string, required = false) {
  return {
    permission_type,
    granted,
    required,
    description: "",
    instructions: "",
  };
}

function state(flags: {
  accessibility?: boolean;
  screen_recording?: boolean;
  microphone?: boolean;
  input_monitoring?: boolean;
}) {
  const accessibility = flags.accessibility ?? true;
  const screen_recording = flags.screen_recording ?? true;
  const microphone = flags.microphone ?? false;
  const input_monitoring = flags.input_monitoring ?? true;
  return {
    accessibility: perm(accessibility, "accessibility", true),
    screen_recording: perm(screen_recording, "screen_recording", true),
    microphone: perm(microphone, "microphone"),
    input_monitoring: perm(input_monitoring, "input_monitoring"),
    all_granted: accessibility && screen_recording && microphone && input_monitoring,
    app_name: "Juno",
  };
}

describe("auto-grant handoff: when to raise the microphone prompt", () => {
  it("prompts when every automatable permission landed and mic is still off", () => {
    expect(shouldAutoPromptMicrophone(state({}))).toBe(true);
  });

  it("does not prompt when the microphone is already granted", () => {
    expect(shouldAutoPromptMicrophone(state({ microphone: true }))).toBe(false);
  });

  it("does not prompt when screen recording failed auto-grant", () => {
    // The failed row is the active manual row; a mic prompt on top of it
    // would be two asks at once.
    expect(shouldAutoPromptMicrophone(state({ screen_recording: false }))).toBe(false);
  });

  it("does not prompt when input monitoring failed auto-grant", () => {
    expect(shouldAutoPromptMicrophone(state({ input_monitoring: false }))).toBe(false);
  });

  it("does not prompt when both automatable permissions failed", () => {
    expect(
      shouldAutoPromptMicrophone(state({ screen_recording: false, input_monitoring: false }))
    ).toBe(false);
  });
});
