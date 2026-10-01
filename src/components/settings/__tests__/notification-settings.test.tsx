import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { COMMANDS } from "@/lib/constants.generated";
import NotificationSettings from "@/components/settings/sections/NotificationSettings";
import type { NotificationStatus } from "@/types/notifications";

/** What Rust reports when it has no reason to think a notification is blocked. */
const MACOS_DECIDES: NotificationStatus = {
  availability: "macos_decides",
  plugin_permission: "granted",
  headline: "macOS decides",
  detail:
    "Juno hands every notification to macOS. Whether one appears is your choice in System Settings > Notifications > Juno, and macOS does not report that choice back to an app, so Juno cannot promise you will see one.",
  can_notify: true,
  system_settings_pane: "notifications",
};

/** What Rust reports from a dev build, where no Juno banner can ever appear. */
const DEV_BUILD: NotificationStatus = {
  availability: "dev_build",
  plugin_permission: "granted",
  headline: "Development build",
  detail:
    "This is a development build. macOS posts its notifications under Terminal rather than Juno, so a Juno banner cannot appear. Test from an installed Juno.",
  can_notify: false,
  system_settings_pane: null,
};

function mockBackend(status: NotificationStatus, test?: () => Promise<unknown>) {
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case COMMANDS.NOTIFICATIONS_GET_NOTIFICATION_SETTINGS:
        return Promise.resolve({ enabled: true });
      case COMMANDS.NOTIFICATIONS_CHECK_NOTIFICATION_PERMISSION:
        return Promise.resolve(status);
      case COMMANDS.NOTIFICATIONS_TEST_NOTIFICATION:
        return test ? test() : Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });
}

describe("the notifications pane says only what Rust knows", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // The reported defect: "if I click test notifications and one, it doesn't
  // show anything, even though it says allowed". The word came from the
  // plugin's desktop permission check, which is a hard-coded `granted`.
  it("never says allowed, and draws the sentence Rust wrote", async () => {
    mockBackend(MACOS_DECIDES);
    render(<NotificationSettings />);

    await waitFor(() => expect(screen.getByText(MACOS_DECIDES.headline)).toBeInTheDocument());
    expect(screen.getByText(MACOS_DECIDES.detail)).toBeInTheDocument();
    expect(screen.queryByText(/allowed/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /ask/i })).not.toBeInTheDocument();
  });

  it("shows why nothing will appear and does not offer a test that cannot work", async () => {
    mockBackend(DEV_BUILD);
    render(<NotificationSettings />);

    await waitFor(() => expect(screen.getByText(DEV_BUILD.detail)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /send one/i })).toBeDisabled();
    // Nothing in that pane holds a row for a dev build, so nobody is sent there.
    expect(screen.queryByRole("button", { name: /open settings/i })).not.toBeInTheDocument();
  });

  // `test_notification` used to call `notify`, discard the result and return
  // `Ok(())` whatever happened, so a refusal reached the screen as success.
  it("shows the reason when the backend refuses to send", async () => {
    mockBackend(MACOS_DECIDES, () => Promise.reject(DEV_BUILD.detail));
    render(<NotificationSettings />);

    await waitFor(() => expect(screen.getByRole("button", { name: /send one/i })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: /send one/i }));

    await waitFor(() => expect(screen.getByText(DEV_BUILD.detail)).toBeInTheDocument());
  });

  // A successful send means Juno handed the notification to macOS, never that
  // anyone saw it: the plugin returns before it posts. The copy says so.
  it("does not claim a sent notification was seen", async () => {
    mockBackend(MACOS_DECIDES);
    render(<NotificationSettings />);

    await waitFor(() => expect(screen.getByRole("button", { name: /send one/i })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: /send one/i }));

    await waitFor(() => expect(screen.getByText(/if nothing appeared/i)).toBeInTheDocument());
  });

  it("opens the one pane where a person can change the answer", async () => {
    mockBackend(MACOS_DECIDES);
    render(<NotificationSettings />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /open settings/i })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /open settings/i }));

    expect(invoke).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_OPEN_SYSTEM_SETTINGS, {
      permission_type: "notifications",
    });
  });
});
