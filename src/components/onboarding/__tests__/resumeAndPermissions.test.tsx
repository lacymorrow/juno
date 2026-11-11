import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

import OnboardingFlow, { resumeStepIndex } from "../Onboarding";
import { COMMANDS, UI } from "@/lib/constants.generated";

const perm = (type: string, granted: boolean) => ({
  permission_type: type,
  granted,
  required: true,
  description: "",
  instructions: "",
});

const permissions = (accessibility: boolean) => ({
  accessibility: perm("accessibility", accessibility),
  screen_recording: perm("screen_recording", false),
  microphone: perm("microphone", false),
  input_monitoring: perm("input_monitoring", false),
  all_granted: false,
  everything_granted: false,
  app_name: "Juno",
});

const slow = { timeout: 5000 };

interface World {
  lastPhase: string | null;
  accessibility: boolean;
  keys: boolean;
}

let world: World;

/** The phases setup recorded, in order. Recording one is what saves the resume point. */
const recordedPhases = () =>
  invoke.mock.calls
    .filter(
      ([cmd, args]) =>
        cmd === COMMANDS.ONBOARDING_RECORD_ONBOARDING_EVENT &&
        args?.eventName === "onboarding_phase_entered",
    )
    .map(([, args]) => args.payload.phase as string);

beforeEach(() => {
  world = {
    lastPhase: null,
    accessibility: false,
    keys: true,
  };
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS:
        return permissions(world.accessibility);
      case COMMANDS.STT_MODELS_GET_STATUS:
        return { offer_recommended: false, models: [] };
      case COMMANDS.PROVIDERS_CHECK_API_KEYS_AVAILABLE:
        return world.keys;
      case COMMANDS.BAR_UI_GET_BAR_CONFIG:
        return { bar_appearance: UI.BAR_APPEARANCES_DEFAULT };
      case COMMANDS.PERMISSIONS_AWAITING_RELAUNCH:
        return [];
      case COMMANDS.ONBOARDING_GET_LAST_ONBOARDING_PHASE:
        return world.lastPhase;
      case COMMANDS.ONBOARDING_CHECK_CLAUDE_CLI_AVAILABLE:
        return { available: false, authenticated: false, email: null };
      case COMMANDS.PERMISSIONS_REQUEST_ACCESSIBILITY_PERMISSION:
        return false;
      default:
        return null;
    }
  });
});

describe("where setup reopens", () => {
  const all = ["welcome", "appearance", "api-key", "permissions", "dictation-model", "complete"];

  it("starts at the beginning when nothing was recorded", () => {
    expect(resumeStepIndex(all, null)).toBe(0);
    expect(resumeStepIndex(all, "a step no build ever had")).toBe(0);
  });

  it("returns to the step that was on screen", () => {
    expect(resumeStepIndex(all, "permissions")).toBe(3);
    expect(resumeStepIndex(["welcome", "appearance", "permissions", "complete"], "permissions")).toBe(2);
  });

  it("moves to the next step that remains when the saved one is finished", () => {
    // Everything was granted before the quit, so the permissions step is gone.
    expect(resumeStepIndex(["welcome", "appearance", "dictation-model", "complete"], "permissions")).toBe(2);
    expect(resumeStepIndex(["welcome", "appearance", "complete"], "permissions")).toBe(2);
  });
});

describe("reopening Juno after macOS asks for a quit and reopen", () => {
  it("comes back to permissions, not the welcome screen", async () => {
    world.lastPhase = "permissions";
    world.accessibility = true;
    render(<OnboardingFlow onComplete={vi.fn()} />);

    await screen.findByText("What Juno can do", {}, slow);
    // Accessibility is on, so the next thing offered is Juno finishing the rest.
    await screen.findByRole("button", { name: "Finish setup for me" }, slow);
  }, 15000);

  it("never saves the welcome step over the step it is about to resume", async () => {
    world.lastPhase = "permissions";
    world.accessibility = true;
    render(<OnboardingFlow onComplete={vi.fn()} />);

    await screen.findByText("What Juno can do", {}, slow);
    await waitFor(() => expect(recordedPhases()).toContain("permissions"));
    expect(recordedPhases()).not.toContain("welcome");
  }, 15000);

  it("still records the welcome step on a first run", async () => {
    render(<OnboardingFlow onComplete={vi.fn()} />);
    await screen.findByRole("button", { name: "Get Started" }, slow);
    await waitFor(() => expect(recordedPhases()).toEqual(["welcome"]));
  }, 15000);
});

describe("changing your mind about doing permissions by hand", () => {
  it("keeps Juno's offer within reach after 'I'll do it myself'", async () => {
    world.lastPhase = "permissions";
    world.accessibility = true;
    render(<OnboardingFlow onComplete={vi.fn()} />);

    const handBack = "Let Juno switch these on instead";
    await screen.findByRole("button", { name: "Finish setup for me" }, slow);
    expect(screen.queryByRole("button", { name: handBack })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "I'll do it myself" }));
    fireEvent.click(await screen.findByRole("button", { name: handBack }, slow));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_AUTO_GRANT_PERMISSIONS, {
        permissions: ["screen_recording", "input_monitoring"],
      }),
    );
    // The run owns the screen now; the link has done its job.
    await waitFor(() => expect(screen.queryByRole("button", { name: handBack })).toBeNull());
  }, 15000);

  it("does not offer it before Accessibility is on, when Juno could not do it", async () => {
    world.lastPhase = "permissions";
    render(<OnboardingFlow onComplete={vi.fn()} />);

    await screen.findByRole("button", { name: "Open Settings for Accessibility" }, slow);
    expect(screen.queryByRole("button", { name: "Let Juno switch these on instead" })).toBeNull();
  }, 15000);
});

describe("the System Settings illustration", () => {
  const caption = "In System Settings, switch Juno on";

  it("is not on the permissions screen until System Settings has been opened", async () => {
    world.lastPhase = "permissions";
    render(<OnboardingFlow onComplete={vi.fn()} />);

    const open = await screen.findByRole("button", { name: "Open Settings for Accessibility" }, slow);
    expect(screen.queryByText(caption)).toBeNull();

    fireEvent.click(open);
    const shown = await screen.findByText(caption, {}, slow);
    // Under the checklist, never above it.
    const row = screen.getByText("Accessibility");
    expect(row.compareDocumentPosition(shown) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  }, 15000);
});

describe("the Connect step", () => {
  it("offers no provider cards beyond the Claude plan Juno can pick up", async () => {
    world.lastPhase = "api-key";
    world.keys = false;
    render(<OnboardingFlow onComplete={vi.fn()} />);

    await screen.findByText("Connect Your AI", {}, slow);
    await screen.findByText("Not installed. Get Claude Code", {}, slow);
    expect(screen.queryByText("Use your ChatGPT plan")).toBeNull();
  }, 15000);
});
