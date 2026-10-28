import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

import OnboardingFlow from "../Onboarding";
import { COMMANDS, UI } from "@/lib/constants.generated";

const granted = (type: string) => ({
  permission_type: type,
  granted: true,
  required: true,
  description: "",
  instructions: "",
});

// Everything granted, a key already available and no model on offer: setup is
// welcome, appearance, and the last screen.
const everythingGranted = {
  accessibility: granted("accessibility"),
  screen_recording: granted("screen_recording"),
  microphone: granted("microphone"),
  input_monitoring: granted("input_monitoring"),
  all_granted: true,
  everything_granted: true,
  app_name: "Juno",
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS:
        return everythingGranted;
      case COMMANDS.STT_MODELS_GET_STATUS:
        return { offer_recommended: false, models: [] };
      case COMMANDS.PROVIDERS_CHECK_API_KEYS_AVAILABLE:
        return true;
      case COMMANDS.BAR_UI_GET_BAR_CONFIG:
        return { bar_appearance: UI.BAR_APPEARANCES_DEFAULT };
      case COMMANDS.PERMISSIONS_AWAITING_RELAUNCH:
        return [];
      default:
        return null;
    }
  });
});

describe("finishing setup", () => {
  it("finishes once however many times Enter is pressed on the last screen", async () => {
    const onComplete = vi.fn();
    render(<OnboardingFlow onComplete={onComplete} />);
    const slow = { timeout: 5000 };

    fireEvent.click(await screen.findByRole("button", { name: "Get Started" }, slow));
    await screen.findByText("Pick how Juno looks", {}, slow);
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText("You're all set", {}, slow);

    // The window outlives the first press by a beat, and the listener with it.
    await act(async () => {
      fireEvent.keyDown(window, { key: "Enter" });
      fireEvent.keyDown(window, { key: "Enter" });
      fireEvent.keyDown(window, { key: "Enter" });
    });

    expect(onComplete).toHaveBeenCalledTimes(1);
  }, 15000);
});
