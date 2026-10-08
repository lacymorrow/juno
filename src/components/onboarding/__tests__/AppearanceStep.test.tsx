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
import { AppearanceStep } from "../AppearanceStep";
import { APPEARANCE_CATALOG } from "@/components/bar/appearanceCatalog";
import { COMMANDS, UI } from "@/lib/constants.generated";

const savedConfig = { bar_appearance: UI.BAR_APPEARANCES_DEFAULT, other: 1 };

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) =>
    cmd === COMMANDS.BAR_UI_GET_BAR_CONFIG ? savedConfig : null,
  );
});

const currentFrame = () =>
  document.querySelector('iframe[data-current="true"]') as HTMLIFrameElement;
const appearanceOf = (f: HTMLIFrameElement) =>
  new URL(f.getAttribute("src") ?? "", "http://x").searchParams.get("appearance");

describe("onboarding appearance step", () => {
  it("preselects the default look, so Continue with no choice is valid", () => {
    render(<AppearanceStep />);
    expect(appearanceOf(currentFrame())).toBe(UI.BAR_APPEARANCES_DEFAULT);
    expect(invoke).not.toHaveBeenCalledWith(COMMANDS.BAR_UI_SET_BAR_CONFIG, expect.anything());
  });

  it("saves through the command Settings uses and keeps the rest of the config", async () => {
    render(<AppearanceStep />);
    const other = APPEARANCE_CATALOG.find((e) => e.value !== UI.BAR_APPEARANCES_DEFAULT)!;
    fireEvent.click(screen.getByRole("tab", { name: other.name }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.BAR_UI_SET_BAR_CONFIG, {
        config: { ...savedConfig, bar_appearance: other.value },
      }),
    );
    expect(appearanceOf(currentFrame())).toBe(other.value);
  });

  it("is the second screen, and Continue advances past it", async () => {
    render(<OnboardingFlow onComplete={() => {}} />);
    // The steps load lazily; a loaded CI runner can take past the 1s default.
    const slow = { timeout: 5000 };
    fireEvent.click(await screen.findByRole("button", { name: "Get Started" }, slow));
    expect(await screen.findByText("Pick how Juno looks", {}, slow)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Skip this step" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(screen.queryByText("Pick how Juno looks")).toBeNull(), slow);
  }, 15000);
});
