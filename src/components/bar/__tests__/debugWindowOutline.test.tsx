import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { EVENTS } from "@/lib/constants.generated";

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
// listen() resolves on a later task, as the real one does. Registering
// synchronously would let an assertion that never waits for it pass here and
// still fail on a loaded runner, which is how this file flaked (LAC-7015).
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (name: string, fn: (event: { payload: unknown }) => void) =>
      new Promise<() => void>((resolve) =>
        setTimeout(() => {
          handlers.set(name, fn);
          resolve(() => handlers.delete(name));
        }, 0),
      ),
  ),
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
    // The look on screen says nothing about this listener. It attaches through
    // useEventListener, whose listen() is a promise, and the outline is a
    // different component from the look: waiting on the look rather than on the
    // registration is what made this flake under CI load (LAC-7015).
    await waitFor(() => expect(handlers.has(EVENTS.BAR_DEBUG_MODE_CHANGED)).toBe(true));
    const changed = handlers.get(EVENTS.BAR_DEBUG_MODE_CHANGED);
    expect(changed).toBeDefined();
    await act(async () => changed?.({ payload: true }));
    expect(screen.getByTestId("debug-window-outline")).toBeTruthy();
    await act(async () => changed?.({ payload: false }));
    expect(screen.queryByTestId("debug-window-outline")).toBeNull();
  });
});
