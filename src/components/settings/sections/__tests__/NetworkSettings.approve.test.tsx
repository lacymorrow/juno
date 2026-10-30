import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { invoke } from "@tauri-apps/api/core";

import NetworkSettings from "../NetworkSettings";
import type { SettingsSectionProps } from "../../types";
import { COMMANDS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);

/** The section only reads the MCP fields off the settings context. */
const settingsStub = {
  mcpLoading: false,
  mcpServers: [],
  mcpServerStatuses: {},
  mcpTools: [],
} as unknown as SettingsSectionProps["settings"];

describe("NetworkSettings: adding an MCP server", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("approves the server it just added, by name", async () => {
    render(<NetworkSettings settings={settingsStub} />);

    fireEvent.change(screen.getByLabelText(/Server Configuration/i), {
      target: { value: JSON.stringify({ "my-server": { command: "npx", args: ["x"] } }) },
    });
    fireEvent.click(screen.getByRole("button", { name: /Add Server/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.MCP_APPROVE_MCP_SERVER, {
        serverName: "my-server",
      }),
    );
    const order = invokeMock.mock.calls.map(([cmd]) => cmd);
    expect(order.indexOf(COMMANDS.MCP_ADD_MCP_SERVER)).toBeLessThan(
      order.indexOf(COMMANDS.MCP_APPROVE_MCP_SERVER),
    );
  });

  it("does not approve when the add fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("store write failed"));
    render(<NetworkSettings settings={settingsStub} />);

    fireEvent.change(screen.getByLabelText(/Server Configuration/i), {
      target: { value: JSON.stringify({ "my-server": { command: "npx" } }) },
    });
    fireEvent.click(screen.getByRole("button", { name: /Add Server/i }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(invokeMock).not.toHaveBeenCalledWith(
      COMMANDS.MCP_APPROVE_MCP_SERVER,
      expect.anything(),
    );
  });
});
