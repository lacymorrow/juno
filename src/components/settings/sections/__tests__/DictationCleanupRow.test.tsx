import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import { DictationCleanupRow } from "../DictationCleanupRow";

const invokeMock = vi.mocked(invoke);

function status(overrides: Record<string, unknown> = {}) {
  return {
    enabled: true,
    availability: "available",
    reason: null,
    can_open_settings: false,
    ...overrides,
  };
}

function answer(value: unknown) {
  invokeMock.mockImplementation(((command: string) =>
    Promise.resolve(
      command === COMMANDS.DICTATION_GET_DICTATION_CLEANUP_STATUS
        ? value
        : undefined,
    )) as typeof invoke);
}

describe("DictationCleanupRow", () => {
  beforeEach(() => invokeMock.mockReset());

  it("shows an enabled switch with one line of copy when the model can run", async () => {
    answer(status());
    render(<DictationCleanupRow />);
    const toggle = await screen.findByRole("switch");
    expect(toggle).toBeEnabled();
    expect(toggle).toBeChecked();
    expect(
      screen.getByText("Fixes grammar and punctuation on your Mac."),
    ).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();
  });

  it("saves a change through the backend", async () => {
    answer(status());
    render(<DictationCleanupRow />);
    fireEvent.click(await screen.findByRole("switch"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.DICTATION_SET_DICTATION_CLEANUP_ENABLED,
        { enabled: false },
      ),
    );
  });

  it("disables the switch and says why when the model is unavailable", async () => {
    answer(
      status({
        availability: "unsupported",
        reason: "Needs macOS 26 on a Mac with Apple silicon.",
      }),
    );
    render(<DictationCleanupRow />);
    const toggle = await screen.findByRole("switch");
    expect(toggle).toBeDisabled();
    expect(toggle).not.toBeChecked();
    expect(
      screen.getByText("Needs macOS 26 on a Mac with Apple silicon."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Open System Settings")).toBeNull();
  });

  it("offers System Settings when Apple Intelligence is off", async () => {
    answer(
      status({
        availability: "apple_intelligence_off",
        reason: "Turn on Apple Intelligence in System Settings to use this.",
        can_open_settings: true,
      }),
    );
    render(<DictationCleanupRow />);
    fireEvent.click(await screen.findByText("Open System Settings"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.DICTATION_OPEN_APPLE_INTELLIGENCE_SETTINGS,
      ),
    );
  });

  it("draws nothing when the backend gives no usable status", async () => {
    answer(undefined);
    const { container } = render(<DictationCleanupRow />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(container.firstChild).toBeNull();
  });
});
