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
      case COMMANDS.TRIGGERS_USE_NO_FN_DEFAULTS:
        // What `triggers::apply_no_fn_defaults` writes.
        return Promise.resolve([
          row("a", "agent", "RightOption"),
          row("d", "dictation", "Control"),
        ]);
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

  it("offers the switch, and it writes Right Option and Control", async () => {
    mountBackend(FN_DEFAULTS, false);
    render(<TriggersSettings settings={settings} />);
    (await screen.findByText("My keyboard has no Fn key")).click();

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.TRIGGERS_USE_NO_FN_DEFAULTS),
    );
    expect(await screen.findByLabelText("Right Option")).toBeTruthy();
    expect(screen.getByLabelText("Control")).toBeTruthy();
    // Nothing left on Fn, so the offer is gone.
    await waitFor(() => expect(screen.queryByText("My keyboard has no Fn key")).toBeNull());
  });

  it("is not offered when nothing is bound to Fn", async () => {
    mountBackend([row("a", "agent", "RightOption"), row("d", "dictation", "Control")], false);
    render(<TriggersSettings settings={settings} />);
    await screen.findByLabelText("Right Option");
    expect(screen.queryByText("My keyboard has no Fn key")).toBeNull();
  });

  it("draws Right Option as one cap", () => {
    expect(shortcutCaps("RightOption").map((c) => c.name)).toEqual(["Right Option"]);
  });
});
