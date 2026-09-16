import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listeners = new Map<string, Array<(payload: unknown) => void>>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (e: { payload: unknown }) => void) => {
    const wrapped = (payload: unknown) => handler({ payload });
    const forEvent = listeners.get(event) ?? [];
    forEvent.push(wrapped);
    listeners.set(event, forEvent);
    return () => {
      listeners.set(
        event,
        (listeners.get(event) ?? []).filter((fn) => fn !== wrapped),
      );
    };
  }),
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import { PermissionNotice } from "../PermissionNotice";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

const ACCESSIBILITY_NEEDED = {
  permission: "accessibility",
  action: "left_click",
  title: "Juno can do that once Accessibility is on",
  detail: "Accessibility lets Juno click and type for you. It is one switch in System Settings.",
};

async function emitNeeded(payload: unknown) {
  await act(async () => {
    for (const handler of listeners.get(EVENTS.PERMISSIONS_NEEDED) ?? []) {
      handler(payload);
    }
  });
}

/** Lets the component's listener attach before the first event is fired. */
async function flush() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("PermissionNotice", () => {
  beforeEach(() => {
    listeners.clear();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({
      accessibility: { granted: false },
      screen_recording: { granted: false },
    });
  });

  it("says nothing until Juno actually needs something", async () => {
    render(<PermissionNotice />);
    await flush();
    expect(screen.queryByTestId("permission-notice")).toBeNull();
  });

  it("asks with the reason attached when a permission is reached for", async () => {
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);

    expect(screen.getByText(ACCESSIBILITY_NEEDED.title)).toBeTruthy();
    expect(screen.getByText(ACCESSIBILITY_NEEDED.detail)).toBeTruthy();
  });

  it("offers Not now as a real answer that dismisses the ask", async () => {
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);

    fireEvent.click(screen.getByText("Not now"));

    await waitFor(() => {
      expect(screen.queryByTestId("permission-notice")).toBeNull();
    });
  });

  it("opens the exact pane for the permission that was needed", async () => {
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);

    fireEvent.click(screen.getByText("Open Settings"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_OPEN_SYSTEM_SETTINGS, {
        permissionType: "accessibility",
      });
    });
  });

  it("does not restart the card when the same ask arrives twice", async () => {
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);
    await emitNeeded({ ...ACCESSIBILITY_NEEDED, detail: "a different sentence" });

    // The first wording stays: re-rendering under someone mid-read is worse
    // than showing slightly staler copy.
    expect(screen.getByText(ACCESSIBILITY_NEEDED.detail)).toBeTruthy();
    expect(screen.queryByText("a different sentence")).toBeNull();
  });

  it("confirms when the switch is flipped instead of just disappearing", async () => {
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);

    // The person goes to System Settings and turns it on; the next poll sees it.
    vi.mocked(invoke).mockResolvedValue({
      accessibility: { granted: true },
      screen_recording: { granted: false },
    });

    await waitFor(
      () => {
        expect(screen.getByText(/Accessibility is on/)).toBeTruthy();
      },
      { timeout: 6000 },
    );
  }, 10000);
});
