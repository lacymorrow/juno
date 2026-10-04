import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

import { COMMANDS } from "@/lib/constants.generated";
import TriggersSettings from "@/components/settings/sections/TriggersSettings";
import { shortcutCaps } from "@/components/settings/KeyCaps";
import type { SettingsSectionProps } from "@/components/settings/types";

type Row = {
  id: string;
  gesture: string;
  target: string;
  binding: { kind: "keyboard"; shortcut: string } | null;
  phrase: string | null;
  require_hey_prefix: boolean;
  enabled: boolean;
};

const row = (id: string, target: string, shortcut: string): Row => ({
  id,
  gesture: "hold",
  target,
  binding: { kind: "keyboard", shortcut },
  phrase: null,
  require_hey_prefix: false,
  enabled: true,
});

const FN_DEFAULTS = [row("a", "agent", "Fn"), row("d", "dictation", "Fn+Control")];
const NOTE = /macOS gives the globe key its own job/;
const settings = { alwaysListeningActive: false } as unknown as SettingsSectionProps["settings"];

function mountBackend(list: Row[], needsSetup: boolean) {
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case COMMANDS.TRIGGERS_GET_TRIGGERS:
        return Promise.resolve(list);
      case COMMANDS.TRIGGERS_GET_TRIGGER_ISSUES:
        return Promise.resolve([]);
      case COMMANDS.TRIGGERS_GLOBE_KEY_NEEDS_SETUP:
        return Promise.resolve(needsSetup);
      default:
        return Promise.resolve(null);
    }
  });
}

describe("the globe key note", () => {
  beforeEach(() => invoke.mockReset());

  it("shows once, not under every Fn row, when the system still needs the change", async () => {
    mountBackend(
      [row("a", "agent", "Fn"), row("d", "dictation", "Fn+Control"), row("x", "agent", "Fn")],
      true,
    );
    render(<TriggersSettings settings={settings} />);
    expect(await screen.findAllByText(NOTE)).toHaveLength(1);
    expect(screen.getByText("System Settings, Keyboard")).toBeTruthy();
  });

  it("is not shown when the key is already set to Do Nothing", async () => {
    mountBackend(FN_DEFAULTS, false);
    render(<TriggersSettings settings={settings} />);
    await screen.findByLabelText("Edit binding for Hold to talk to Juno");
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.TRIGGERS_GLOBE_KEY_NEEDS_SETUP),
    );
    expect(screen.queryByText(NOTE)).toBeNull();
  });
});

describe("a keyboard with no Fn key", () => {
  beforeEach(() => invoke.mockReset());

  it("is never a question: the screen has no such switch", async () => {
    mountBackend(FN_DEFAULTS, false);
    render(<TriggersSettings settings={settings} />);
    await screen.findByLabelText("Edit binding for Hold to talk to Juno");
    expect(screen.queryByText(/no Fn key/i)).toBeNull();
  });

  it("shows the keys the backend moved the defaults to", async () => {
    // What `triggers::settle_for_keyboard` leaves when no globe key is
    // connected.
    mountBackend([row("a", "agent", "RightOption"), row("d", "dictation", "Control")], false);
    render(<TriggersSettings settings={settings} />);
    expect(await screen.findByLabelText("Right Option")).toBeTruthy();
    expect(screen.getByLabelText("Control")).toBeTruthy();
  });

  it("draws Right Option as one cap", () => {
    expect(shortcutCaps("RightOption").map((c) => c.name)).toEqual(["Right Option"]);
  });
});

describe("modifier chords", () => {
  beforeEach(() => invoke.mockReset());

  it("draws each modifier of a chord as its own cap", () => {
    expect(shortcutCaps("Control+Option").map((c) => c.name)).toEqual(["Control", "Option"]);
    expect(shortcutCaps("Fn+Shift").map((c) => c.name)).toEqual(["Shift", "Globe"]);
  });

  it("lists a globe chord beside the globe key", async () => {
    mountBackend([row("a", "agent", "Fn"), row("d", "dictation", "Fn+Option")], false);
    render(<TriggersSettings settings={settings} />);
    expect(await screen.findByLabelText("Globe")).toBeTruthy();
    expect(screen.getByLabelText("Option Globe")).toBeTruthy();
  });
});
