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

  it("authorized: the sample is an icon beside the switch, and toggling sends nothing", async () => {
    mockBackend(status("authorized"));
    render(<NotificationsGroup />);

    const toggle = await screen.findByRole("switch");
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).toBeChecked();
    expect(screen.queryByRole("button", { name: /send one/i })).not.toBeInTheDocument();

    // Off then on sends nothing, and off hides the sample.
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(
      screen.queryByRole("button", { name: /send a sample notification/i }),
    ).not.toBeInTheDocument();
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(invoke).not.toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);

    // The icon is the one way to send one.
    const sample = screen.getByRole("button", { name: /send a sample notification/i });
    fireEvent.click(sample);
    expect(invoke).toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);
    await waitFor(() => expect(sample).toBeEnabled());
  });

  it("not determined: allowing sends nothing on its own", async () => {
    mockBackend(status("not_determined"));
    render(<NotificationsGroup />);
    fireEvent.click(await screen.findByRole("button", { name: /allow notifications/i }));
    await waitFor(() => expect(screen.getByRole("switch")).toBeEnabled());
    expect(invoke).not.toHaveBeenCalledWith(COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION);
  });

  it("authorized: a failed send shows no error, and the row re-reads macOS", async () => {
    let checks = 0;
    invoke.mockImplementation((command: string) => {
      if (command === COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS)
        return Promise.resolve({ enabled: true });
      if (command === COMMANDS.NOTIFICATIONS_CHECK_NOTIFICATION_PERMISSION) {
        checks += 1;
        return Promise.resolve(status(checks === 1 ? "authorized" : "denied"));
      }
      if (command === COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION)
        return Promise.reject("macOS would not take the notification.");
      return Promise.resolve(null);
    });
    render(<NotificationsGroup />);

    fireEvent.click(
      await screen.findByRole("button", { name: /send a sample notification/i }),
    );
    await screen.findByRole("button", { name: /open system settings/i });
    expect(
      screen.queryByText("macOS would not take the notification."),
    ).not.toBeInTheDocument();
  });
});
