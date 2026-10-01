import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { COMMANDS } from "@/lib/constants.generated";
import TriggersSettings from "@/components/settings/sections/TriggersSettings";
import type { SettingsSectionProps } from "@/components/settings/types";

/** The message the backend raises when two rows fight over the globe key. */
const CONFLICT = '"Fn (globe)" already has Hold to talk to Juno.';

type Row = {
  id: string;
  gesture: string;
  target: string;
  binding: { kind: "keyboard"; shortcut: string } | null;
  phrase: string | null;
  require_hey_prefix: boolean;
  enabled: boolean;
};

const row = (id: string, gesture: string, target: string, shortcut: string): Row => ({
  id,
  gesture,
  target,
  binding: { kind: "keyboard", shortcut },
  phrase: null,
  require_hey_prefix: false,
  enabled: true,
});

/** Two rows on one key, the state the person is looking at when it breaks. */
const STORED: Row[] = [
  row("row-hold", "hold", "agent", "Fn"),
  row("row-tap", "tap", "dictation", "Fn"),
];

const settings = {
  alwaysListeningActive: false,
} as unknown as SettingsSectionProps["settings"];

function renderScreen() {
  return render(<TriggersSettings settings={settings} />);
}

describe("a trigger error does not outlive its cause", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("clears a conflict message once the conflict is resolved from another row", async () => {
    // The reported defect: the refusal was dropped only when the row being
    // edited happened to be the row the message was pinned to. Fixing the
    // conflict from anywhere else left "'Fn (globe)' is bound to more than one
    // trigger" on screen for the rest of the session.
    let saves = 0;
    invoke.mockImplementation((command: string) => {
      switch (command) {
        case COMMANDS.TRIGGERS_GET_TRIGGERS:
          return Promise.resolve(STORED);
        case COMMANDS.TRIGGERS_SET_TRIGGERS:
          saves += 1;
          // The first save is refused; the second is the person having fixed
          // it, which the backend accepts.
          return saves === 1
            ? Promise.reject(CONFLICT)
            : Promise.resolve([STORED[0]]);
        default:
          return Promise.resolve(null);
      }
    });

    renderScreen();
    await screen.findByLabelText("Enable Hold to talk to Juno");

    // Refuse a save: switching the second row on is rejected for the conflict.
    const tapSwitch = screen.getByLabelText("Enable Tap to dictate");
    tapSwitch.click();
    expect(await screen.findByText(CONFLICT)).toBeTruthy();

    // Now resolve it from the *other* row: delete the one holding the key.
    screen.getByLabelText("Delete Hold to talk to Juno").click();

    await waitFor(() => {
      expect(screen.queryByText(CONFLICT)).toBeNull();
    });
  });

  it("clears a conflict message when the list is reloaded", async () => {
    // Whatever the backend hands back is a list it accepted, so no refusal
    // about it can still be true.
    let saves = 0;
    invoke.mockImplementation((command: string) => {
      switch (command) {
        case COMMANDS.TRIGGERS_GET_TRIGGERS:
          return Promise.resolve(STORED);
        case COMMANDS.TRIGGERS_SET_TRIGGERS:
          saves += 1;
          return Promise.reject(CONFLICT);
        default:
          return Promise.resolve(null);
      }
    });

    const { unmount } = renderScreen();
    await screen.findByLabelText("Enable Tap to dictate");
    screen.getByLabelText("Enable Tap to dictate").click();
    expect(await screen.findByText(CONFLICT)).toBeTruthy();
    expect(saves).toBe(1);
    unmount();

    renderScreen();
    await screen.findByLabelText("Enable Tap to dictate");
    expect(screen.queryByText(CONFLICT)).toBeNull();
  });
});

describe("a row says how its own gesture ends", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((command: string) => {
      if (command === COMMANDS.TRIGGERS_GET_TRIGGERS) {
        return Promise.resolve([
          row("row-hold", "hold", "agent", "Fn"),
          row("row-dth", "double_tap_hold", "dictation", "Fn"),
        ]);
      }
      return Promise.resolve(null);
    });
  });

  it("never describes a second gesture reaching the same target", async () => {
    // The copy the user quoted: "Hold the key while you speak, let go to
    // finish. Double-tap it to keep listening until you press it again." That
    // behaviour is deleted, so no row may still describe it.
    renderScreen();
    await screen.findByLabelText("Enable Hold to talk to Juno");

    expect(screen.getAllByText("Let go to finish.")).toHaveLength(2);
    expect(
      screen.queryByText(/Double-tap it to keep listening/i),
    ).toBeNull();
  });

  it("reads Hold and Double-tap and hold as two rows on one key", async () => {
    renderScreen();
    expect(await screen.findByText("Hold")).toBeTruthy();
    expect(screen.getByText("Double-tap and hold")).toBeTruthy();
    expect(screen.getByText("to talk to Juno")).toBeTruthy();
    expect(screen.getByText("to dictate")).toBeTruthy();
  });
});
