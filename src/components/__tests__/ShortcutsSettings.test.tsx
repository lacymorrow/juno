import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import ShortcutsSettings from "../settings/sections/ShortcutsSettings";

// Mock Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

// Radix Popover positions its content with floating-ui, which needs ResizeObserver.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const VOICE_DESCRIPTION =
  "Toggle voice recording from anywhere — no Juno window required";

function makeSettings(overrides: Record<string, unknown> = {}) {
  return {
    keyboardShortcuts: {
      agent_mode: "Option+D",
      dictation_input: "Option+Space",
      stop_current_task: "Escape",
      open_settings: "Cmd+Comma",
      voice_activation: "Option+Shift+V",
    },
    shortcutsLoading: false,
    loadKeyboardShortcuts: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  } as any;
}

describe("ShortcutsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (globalThis as any).ResizeObserver = ResizeObserverStub;
    mockInvoke.mockResolvedValue("OK" as any);
  });

  it("renders every shortcut name with its chip and no inline descriptions", () => {
    render(<ShortcutsSettings settings={makeSettings()} />);

    for (const label of [
      "Agent Mode",
      "Start Dictation",
      "Stop Current Task",
      "Voice Activation",
      "Cancel Current Operation",
      "Open Settings",
    ]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }

    expect(screen.getByText("Option+D")).toBeInTheDocument();
    expect(screen.getByText("Option+Shift+V")).toBeInTheDocument();
    expect(screen.getByText("Cmd+Comma")).toBeInTheDocument();

    // Descriptions are progressively disclosed: none rendered by default.
    expect(screen.queryByText(VOICE_DESCRIPTION)).not.toBeInTheDocument();
    expect(screen.queryByText("Activate agent mode")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Stop any running AI task or operation")
    ).not.toBeInTheDocument();
  });

  it("shows a shortcut's description only after its info icon is activated", async () => {
    render(<ShortcutsSettings settings={makeSettings()} />);

    expect(screen.queryByText(VOICE_DESCRIPTION)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "About Voice Activation" }));

    expect(await screen.findByText(VOICE_DESCRIPTION)).toBeInTheDocument();
    // Other rows' descriptions stay hidden.
    expect(screen.queryByText("Activate agent mode")).not.toBeInTheDocument();
  });

  it("keeps the tips block collapsed until its trigger is clicked", async () => {
    render(<ShortcutsSettings settings={makeSettings()} />);

    const tip = "Changes are applied immediately and saved automatically";
    expect(screen.queryByText(tip)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /keyboard shortcut tips/i }));

    expect(await screen.findByText(tip)).toBeInTheDocument();
  });

  it("resets shortcuts to defaults through the backend", async () => {
    const settings = makeSettings();
    render(<ShortcutsSettings settings={settings} />);

    fireEvent.click(screen.getByRole("button", { name: /reset to defaults/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("reset_keyboard_shortcuts");
      expect(settings.loadKeyboardShortcuts).toHaveBeenCalled();
    });
  });

  it("saves an edited shortcut through set_keyboard_shortcut", async () => {
    const settings = makeSettings();
    render(<ShortcutsSettings settings={settings} />);

    fireEvent.click(
      screen.getByRole("button", { name: "Edit Voice Activation shortcut" })
    );

    const input = screen.getByPlaceholderText(/Ctrl\+Shift\+F1/);
    fireEvent.change(input, { target: { value: "Cmd+Shift+V" } });
    fireEvent.click(screen.getByRole("button", { name: /^save$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_keyboard_shortcut", {
        shortcutName: "voice_activation",
        shortcut: "Cmd+Shift+V",
      });
      expect(settings.loadKeyboardShortcuts).toHaveBeenCalled();
    });
  });

  it("does not render an info trigger for rows without a description", () => {
    render(
      <ShortcutsSettings
        settings={makeSettings({
          keyboardShortcuts: { custom_thing: "F5" },
        })}
      />
    );

    expect(screen.getByText("custom_thing")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "About custom_thing" })
    ).not.toBeInTheDocument();
  });
});
