import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("@/lib/permissions-service", () => ({
  getPermissionsStatus: vi.fn(async () => ({ all_granted: true })),
  invalidatePermissionsCache: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import SecuritySettings from "../SecuritySettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import { COMMANDS } from "@/lib/constants.generated";
import { PERMISSION_MODES } from "@/lib/permissions";

const invokeMock = vi.mocked(invoke);

function mockBackend(storedMode: unknown = "ask_when_risky") {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.TOOLS_GET_PERMISSION_MODE) return storedMode;
    if (command === COMMANDS.TOOLS_SET_PERMISSION_MODE) return storedMode;
    if (command === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED) return false;
    return undefined;
  });
}

function renderSection() {
  return render(
    <AdvancedSettingsProvider>
      <SecuritySettings />
    </AdvancedSettingsProvider>,
  );
}

describe("Security and Privacy: when Juno needs permission", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.mocked(toast.error).mockClear();
  });

  it("offers three named modes, each saying what it permits", async () => {
    mockBackend();
    renderSection();

    for (const mode of PERMISSION_MODES) {
      expect(await screen.findByText(mode.name)).toBeInTheDocument();
      // The whole complaint was that permissions were "scary or opaque". A
      // mode with no stated consequence is opaque by construction.
      expect(screen.getByText(mode.consequence)).toBeInTheDocument();
    }
    expect(PERMISSION_MODES).toHaveLength(3);
  });

  it("says out loud what Juno never asks about, and what it always asks about", async () => {
    mockBackend();
    renderSection();

    const footer = await screen.findByText(/never asks about commands/i);
    expect(footer).toHaveTextContent(/ls, pwd, which, date and sleep/);
    // The floor. It applies in every mode, including the most permissive, so
    // the screen has to say so rather than let someone infer "Don't ask" means
    // "never asks".
    expect(footer).toHaveTextContent(/always asks before something it cannot undo/);
  });

  it("shows the stored mode as selected", async () => {
    mockBackend("dont_ask");
    renderSection();

    await waitFor(() =>
      expect(screen.getByRole("radio", { name: /Don't ask/ })).toBeChecked(),
    );
  });

  it("falls back to asking about risky things when the stored value is unknown", async () => {
    // A hand-edited or newer settings file must not land someone in a mode
    // that asks less than they chose.
    mockBackend("full_autonomy");
    renderSection();

    await waitFor(() =>
      expect(
        screen.getByRole("radio", { name: /Ask about risky things/ }),
      ).toBeChecked(),
    );
  });

  it("saves the mode the person picks", async () => {
    mockBackend();
    renderSection();

    const askFirst = await screen.findByRole("radio", { name: /Ask me first/ });
    fireEvent.click(askFirst);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.TOOLS_SET_PERMISSION_MODE, {
        mode: "ask_first",
      }),
    );
    expect(askFirst).toBeChecked();
  });

  it("puts the old mode back and says so when the save fails", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === COMMANDS.TOOLS_GET_PERMISSION_MODE) return "ask_when_risky";
      if (command === COMMANDS.TOOLS_SET_PERMISSION_MODE) throw new Error("nope");
      return undefined;
    });
    renderSection();

    fireEvent.click(await screen.findByRole("radio", { name: /Don't ask/ }));

    await waitFor(() => expect(toast.error).toHaveBeenCalled());
    expect(
      screen.getByRole("radio", { name: /Ask about risky things/ }),
    ).toBeChecked();
  });
});
