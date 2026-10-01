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
  primary_action: "open_settings" as const,
  primary_label: "Open Settings",
};

const SCREEN_RECORDING_NEEDED = {
  permission: "screen_recording",
  action: "screenshot",
  title: "Juno can see your screen once Screen Recording is on",
  detail: "Screen Recording is how Juno sees what is in front of you.",
  primary_action: "open_settings" as const,
  primary_label: "Open Settings",
};

const AUTOMATION_NEEDED = {
  permission: "automation_system_events",
  action: "turn on dark mode",
  title: "Juno can press keys and work menus for you once you allow it",
  detail: "macOS will ask whether Juno may control System Events. It asks once.",
  primary_action: "allow_automation" as const,
  primary_label: "Allow",
};

/** The shape `permission_moment` answers with. */
function moment(over: Partial<Record<string, unknown>> = {}) {
  return {
    permission: "accessibility",
    granted: false,
    restart_unblocks: false,
    restart_detail: "",
    ...over,
  };
}

/** Answer each command separately, the way the backend does. */
function backend(momentAnswer: Record<string, unknown>, extra: Record<string, unknown> = {}) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === COMMANDS.PERMISSIONS_PERMISSION_MOMENT) return momentAnswer;
    if (command in extra) return extra[command];
    return undefined;
  });
}

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
    backend(moment());
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
    backend(moment({ granted: true }));

    await waitFor(
      () => {
        expect(screen.getByText(/Accessibility is on/)).toBeTruthy();
      },
      { timeout: 6000 },
    );
  }, 10000);

  // ── The restart offer ──────────────────────────────────────────────────────

  it("does not mention a restart before the person has done their part", async () => {
    // Screen Recording owes a restart the moment Juno has been refused, but
    // saying so before they have touched the switch is Juno demanding a
    // restart for something they have not granted yet.
    backend(
      moment({
        permission: "screen_recording",
        restart_unblocks: true,
        restart_detail: "macOS gives an app the screen only when the app starts.",
      }),
    );
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(SCREEN_RECORDING_NEEDED);

    await new Promise((resolve) => setTimeout(resolve, 2200));
    expect(screen.queryByTestId("permission-restart")).toBeNull();
    expect(screen.getByTestId("permission-primary").textContent).toBe("Open Settings");
  }, 10000);

  it("offers one restart button once macOS cannot reach this process", async () => {
    backend(
      moment({
        permission: "screen_recording",
        restart_unblocks: true,
        restart_detail: "macOS gives an app the screen only when the app starts.",
      }),
    );
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(SCREEN_RECORDING_NEEDED);

    fireEvent.click(screen.getByTestId("permission-primary"));

    await waitFor(
      () => {
        expect(screen.getByTestId("permission-restart").textContent).toBe("Restart Juno");
      },
      { timeout: 6000 },
    );
    // The explanation replaces the ask rather than stacking under it.
    expect(screen.getByText(/only when the app starts/)).toBeTruthy();
    expect(screen.queryByText(SCREEN_RECORDING_NEEDED.detail)).toBeNull();
    // "Not now" survives: a restart is still an offer.
    expect(screen.getByText("Not now")).toBeTruthy();
  }, 10000);

  it("never offers a restart for a permission that does not need one", async () => {
    // Accessibility is read live, so a restart would be pure cost.
    backend(moment({ permission: "accessibility", restart_unblocks: false }));
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(ACCESSIBILITY_NEEDED);

    fireEvent.click(screen.getByTestId("permission-primary"));
    await new Promise((resolve) => setTimeout(resolve, 2200));

    expect(screen.queryByTestId("permission-restart")).toBeNull();
  }, 10000);

  it("restarts Juno through the backend when asked to", async () => {
    backend(
      moment({
        permission: "screen_recording",
        restart_unblocks: true,
        restart_detail: "macOS gives an app the screen only when the app starts.",
      }),
    );
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(SCREEN_RECORDING_NEEDED);
    fireEvent.click(screen.getByTestId("permission-primary"));

    await waitFor(() => expect(screen.queryByTestId("permission-restart")).not.toBeNull(), {
      timeout: 6000,
    });
    fireEvent.click(screen.getByTestId("permission-restart"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_RESTART_AFTER_PERMISSIONS);
    });
  }, 10000);

  // ── Automation ─────────────────────────────────────────────────────────────

  it("owns the Automation moment instead of letting macOS ask cold", async () => {
    backend(moment({ permission: "automation_system_events" }), {
      [COMMANDS.PERMISSIONS_ALLOW_AUTOMATION]: true,
    });
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(AUTOMATION_NEEDED);

    // Juno's sentence first, and its button says Allow, not Open Settings:
    // there is no Automation row to open until macOS has asked once.
    expect(screen.getByText(AUTOMATION_NEEDED.detail)).toBeTruthy();
    const button = screen.getByTestId("permission-primary");
    expect(button.textContent).toBe("Allow");

    fireEvent.click(button);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_ALLOW_AUTOMATION, {
        target: "system_events",
      });
    });
    await waitFor(() => {
      expect(screen.getByText(/Juno can control System Events now/)).toBeTruthy();
    });
  }, 10000);

  it("says so plainly when macOS refuses the Automation request", async () => {
    backend(moment({ permission: "automation_system_events" }), {
      [COMMANDS.PERMISSIONS_ALLOW_AUTOMATION]: false,
    });
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(AUTOMATION_NEEDED);

    fireEvent.click(screen.getByTestId("permission-primary"));

    await waitFor(() => {
      expect(screen.getByTestId("permission-error").textContent).toMatch(/did not allow it/);
    });
  }, 10000);

  it("hands the person no file path, bundle id or script in any state", async () => {
    // Lacy's bar: some people do not know what a filepath is.
    backend(
      moment({
        permission: "screen_recording",
        restart_unblocks: true,
        restart_detail: "macOS gives an app the screen only when the app starts.",
      }),
    );
    render(<PermissionNotice />);
    await flush();
    await emitNeeded(SCREEN_RECORDING_NEEDED);
    fireEvent.click(screen.getByTestId("permission-primary"));
    await waitFor(() => expect(screen.queryByTestId("permission-restart")).not.toBeNull(), {
      timeout: 6000,
    });

    const text = screen.getByTestId("permission-notice").textContent ?? "";
    for (const forbidden of ["com.apple", "/Users", "/Applications", "tell application", "~/"]) {
      expect(text).not.toContain(forbidden);
    }
  }, 10000);
});
