import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

const perm = (type: string) => ({
  permission_type: type,
  granted: false,
  required: true,
  description: "",
  instructions: "",
});

const KEY = "sk-ant-api03-" + "a1B2".repeat(10);
const slow = { timeout: 5000 };

/** What the provider says about the key in this test. */
let verdict: string;

beforeEach(() => {
  verdict = "ok";
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS:
        return {
          accessibility: perm("accessibility"),
          screen_recording: perm("screen_recording"),
          microphone: perm("microphone"),
          input_monitoring: perm("input_monitoring"),
          all_granted: false,
          everything_granted: false,
          app_name: "Juno",
        };
      case COMMANDS.STT_MODELS_GET_STATUS:
        return { offer_recommended: false, models: [] };
      case COMMANDS.PROVIDERS_CHECK_API_KEYS_AVAILABLE:
        return false;
      case COMMANDS.BAR_UI_GET_BAR_CONFIG:
        return { bar_appearance: UI.BAR_APPEARANCES_DEFAULT };
      case COMMANDS.PERMISSIONS_AWAITING_RELAUNCH:
        return [];
      case COMMANDS.ONBOARDING_GET_LAST_ONBOARDING_PHASE:
        return "api-key";
      case COMMANDS.ONBOARDING_CHECK_CLAUDE_CLI_AVAILABLE:
        return { available: false, authenticated: false, email: null };
      case COMMANDS.PROVIDERS_CHECK_PROVIDER_API_KEY:
        return verdict;
      default:
        return null;
    }
  });
});

const openStep = async () => {
  render(<OnboardingFlow onComplete={() => {}} />);
  await screen.findByText("Connect Your AI", {}, slow);
  return screen.getByLabelText("Key") as HTMLInputElement;
};

/** Typing or editing: the box's value changes with no paste event. */
const paste = (box: HTMLInputElement, text: string) =>
  fireEvent.change(box, { target: { value: text } });

const continueButton = () => screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement;

describe("pasting a key in setup", () => {
  it("takes the whole message and keeps only the key", async () => {
    const box = await openStep();
    fireEvent.paste(box, {
      clipboardData: { getData: () => `Hi! Here is your key for Juno:\n${KEY}\nDon't share it.` },
    });

    expect(box.value).toBe(KEY);
    await screen.findByText("Your Anthropic key works.", {}, slow);
    expect(invoke).toHaveBeenCalledWith(COMMANDS.PROVIDERS_CHECK_PROVIDER_API_KEY, {
      providerId: "anthropic",
      apiKey: KEY,
    });
    expect(continueButton().disabled).toBe(false);
  });

  it("says so in plain words when the key is turned down, and does not move on", async () => {
    verdict = "rejected";
    const box = await openStep();
    paste(box, KEY);

    await screen.findByText(/This key isn't working/, {}, slow);
    expect(continueButton().disabled).toBe(true);
  });

  it("says when the account behind the key is out of credit", async () => {
    verdict = "no_credit";
    const box = await openStep();
    paste(box, KEY);

    await screen.findByText(/no credit left/, {}, slow);
  });

  it("does not hold an unanswered check against the key", async () => {
    verdict = "unknown";
    const box = await openStep();
    paste(box, KEY);

    await screen.findByText("Anthropic key found.", {}, slow);
    expect(continueButton().disabled).toBe(false);
  });

  it("asks for the whole message when it finds no key", async () => {
    const box = await openStep();
    paste(box, "the key is in my other email");

    await screen.findByText(/Juno can't find a key in that/, {}, slow);
    await waitFor(() => expect(continueButton().disabled).toBe(true));
    expect(invoke).not.toHaveBeenCalledWith(
      COMMANDS.PROVIDERS_CHECK_PROVIDER_API_KEY,
      expect.anything(),
    );
  });
});

describe("a key typed by hand", () => {
  it("is tried once after a pause, not once per keystroke", async () => {
    const box = await openStep();
    for (const end of [KEY.length - 2, KEY.length - 1, KEY.length]) {
      paste(box, KEY.slice(0, end));
    }

    await screen.findByText("Your Anthropic key works.", {}, slow);
    const checks = invoke.mock.calls.filter(
      ([cmd]) => cmd === COMMANDS.PROVIDERS_CHECK_PROVIDER_API_KEY,
    );
    expect(checks).toHaveLength(1);
    expect(checks[0][1]).toEqual({ providerId: "anthropic", apiKey: KEY });
  });
});
