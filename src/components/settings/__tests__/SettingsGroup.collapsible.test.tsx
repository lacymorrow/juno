import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => false) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { SettingsGroup, SettingsRow } from "../ui";
import { revealRow } from "../revealRow";
import { AdvancedSettingsProvider } from "../AdvancedSettingsContext";

function Folded(props: { defaultOpen?: boolean; summary?: string }) {
  return (
    <AdvancedSettingsProvider>
      <SettingsGroup title="Developer Options" collapsible {...props}>
        <SettingsRow htmlFor="debug-mode" label="Debug Mode" />
      </SettingsGroup>
    </AdvancedSettingsProvider>
  );
}

describe("collapsible SettingsGroup", () => {
  it("starts closed with the rows mounted but hidden, and opens on click", () => {
    render(<Folded summary="Debug Mode is on" />);
    const toggle = screen.getByRole("button", { name: /Developer Options/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Debug Mode is on")).toBeInTheDocument();
    expect(document.getElementById("settings-row-debug-mode")).not.toBeVisible();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(document.getElementById("settings-row-debug-mode")).toBeVisible();
    // The summary is for the closed state only.
    expect(screen.queryByText("Debug Mode is on")).not.toBeInTheDocument();
  });

  it("starts open when something inside needs attention", () => {
    render(<Folded defaultOpen />);
    expect(screen.getByRole("button", { name: /Developer Options/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  it("opens when search or the agent reveals a row inside it", () => {
    vi.useFakeTimers();
    const raf = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((cb) => setTimeout(() => cb(0), 0) as unknown as number);
    render(<Folded />);
    const row = document.getElementById("settings-row-debug-mode")!;
    row.scrollIntoView = vi.fn();

    revealRow("debug-mode");
    // One frame asks the group to open; React commits; the next frame scrolls.
    act(() => {
      vi.advanceTimersByTime(1);
    });
    act(() => {
      vi.advanceTimersByTime(1);
    });

    expect(screen.getByRole("button", { name: /Developer Options/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(row.scrollIntoView).toHaveBeenCalled();
    raf.mockRestore();
    vi.useRealTimers();
  });
});
