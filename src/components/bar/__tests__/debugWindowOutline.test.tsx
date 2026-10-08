import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Debug mode outlines the bar window, so a native resize or a transition that
 * moves the window is visible against the desktop. Off by default, and it
 * follows the settings toggle without a relaunch.
 */

const { invoke, handlers } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(null)),
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, fn: (event: { payload: unknown }) => void) => {
    handlers.set(name, fn);
    return () => handlers.delete(name);
  }),
  emit: vi.fn(async () => {}),
}));
vi.mock("@/components/FloatingBar", () => ({ FloatingBar: () => <div data-testid="look" /> }));

import { BarHost } from "../BarHost";

function mount(debug: boolean) {
  invoke.mockImplementation((command: unknown) =>
    Promise.resolve(
      command === "get_debug_mode"
        ? debug
        : command === "ui_get_bar_config"
          ? { bar_appearance: "floating", show_glow_border: true }
          : null,
    ),
  );
  render(<BarHost />);
}

describe("debug window outline", () => {
  beforeEach(() => {
    invoke.mockReset();
    handlers.clear();
  });

  it("is absent when debug mode is off", async () => {
    mount(false);
    await screen.findByTestId("look");
    expect(screen.queryByTestId("debug-window-outline")).toBeNull();
  });

  it("draws a 1px black border around the window when debug mode is on", async () => {
    mount(true);
    const outline = await screen.findByTestId("debug-window-outline");
    expect(outline.style.border).toBe("1px solid rgb(0, 0, 0)");
    expect(outline.style.pointerEvents).toBe("none");
    expect(outline.style.position).toBe("fixed");
  });

  it("follows the settings toggle without a relaunch", async () => {
    mount(false);
    await screen.findByTestId("look");
    const changed = handlers.get("bar-debug-mode-changed");
    expect(changed).toBeDefined();
    await act(async () => changed?.({ payload: true }));
    expect(screen.getByTestId("debug-window-outline")).toBeTruthy();
    await act(async () => changed?.({ payload: false }));
    expect(screen.queryByTestId("debug-window-outline")).toBeNull();
  });
});
