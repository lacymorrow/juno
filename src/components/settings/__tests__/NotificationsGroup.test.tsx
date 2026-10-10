import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { COMMANDS } from "@/lib/constants.generated";
import NotificationsGroup from "@/components/settings/NotificationsGroup";
import type { NotificationAuthorization, NotificationStatus } from "@/types/notifications";

const status = (authorization: NotificationAuthorization): NotificationStatus => ({
  authorization,
  unavailable_reason: null,
});

function mockBackend(initial: NotificationStatus) {
  let current = initial;
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS:
        return Promise.resolve({ enabled: true });
      case COMMANDS.NOTIFICATIONS_CHECK_NOTIFICATION_PERMISSION:
        return Promise.resolve(current);
      case COMMANDS.NOTIFICATIONS_REQUEST_NOTIFICATION_PERMISSION:
        current = status("authorized");
        return Promise.resolve(current);
      default:
        return Promise.resolve(null);
    }
  });
  return (next: NotificationStatus) => {
    current = next;
  };
}

describe("the notifications row shows what macOS allows", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("not determined: the one action asks macOS, and the switch stays off", async () => {
    mockBackend(status("not_determined"));
    render(<NotificationsGroup />);

    const allow = await screen.findByRole("button", { name: /allow notifications/i });
    expect(screen.getByRole("switch")).toBeDisabled();
    expect(screen.getByRole("switch")).not.toBeChecked();
    expect(screen.queryByRole("button", { name: /send one/i })).not.toBeInTheDocument();

    fireEvent.click(allow);
    expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_REQUEST_NOTIFICATION_PERMISSION);
    // The answer comes back and the row turns live.
    await waitFor(() => expect(screen.getByRole("switch")).toBeEnabled());
  });

  it("denied: one line, a button that opens Juno's pane, and a switch that reads off", async () => {
    mockBackend(status("denied"));
    render(<NotificationsGroup />);

    expect(
      await screen.findByText("Notifications are off for Juno in System Settings."),
    ).toBeInTheDocument();
    // Juno's own setting is on, but the system has them off: never "enabled".
    expect(screen.getByRole("switch")).toBeDisabled();
    expect(screen.getByRole("switch")).not.toBeChecked();
    expect(screen.queryByRole("button", { name: /send one/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open system settings/i }));
    expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_OPEN_NOTIFICATION_SETTINGS);
  });

  it("denied: coming back to the window re-reads the answer", async () => {
    const setStatus = mockBackend(status("denied"));
    render(<NotificationsGroup />);
    await screen.findByRole("button", { name: /open system settings/i });

    setStatus(status("authorized"));
    fireEvent.focus(window);

    await waitFor(() => expect(screen.getByRole("switch")).toBeEnabled());
    expect(screen.getByRole("switch")).toBeChecked();
  });

  it("authorized: no test button; turning the switch on sends one", async () => {
    mockBackend(status("authorized"));
    render(<NotificationsGroup />);

    const toggle = await screen.findByRole("switch");
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).toBeChecked();
    expect(screen.queryByRole("button", { name: /send one/i })).not.toBeInTheDocument();

    // Off sends nothing.
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(invoke).not.toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);

    // On sends one: the banner shows what to expect.
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION),
    );
    expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_SET_NOTIFICATIONS_ENABLED, {
      enabled: true,
    });
  });

  it("not determined: allowing sends one", async () => {
    mockBackend(status("not_determined"));
    render(<NotificationsGroup />);
    fireEvent.click(await screen.findByRole("button", { name: /allow notifications/i }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION),
    );
  });

  it("authorized: a failed send shows no error, and the row re-reads macOS", async () => {
    let checks = 0;
    invoke.mockImplementation((command: string) => {
      if (command === COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS)
        return Promise.resolve({ enabled: false });
      if (command === COMMANDS.NOTIFICATIONS_CHECK_NOTIFICATION_PERMISSION) {
        checks += 1;
        return Promise.resolve(status(checks === 1 ? "authorized" : "denied"));
      }
      if (command === COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION)
        return Promise.reject("macOS would not take the notification.");
      return Promise.resolve(null);
    });
    render(<NotificationsGroup />);

    const toggle = await screen.findByRole("switch");
    await waitFor(() => expect(toggle).toBeEnabled());
    fireEvent.click(toggle);
    await screen.findByRole("button", { name: /open system settings/i });
    expect(
      screen.queryByText("macOS would not take the notification."),
    ).not.toBeInTheDocument();
  });
});
