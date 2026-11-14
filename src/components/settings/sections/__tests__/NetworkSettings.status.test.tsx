import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }));

import NetworkSettings from "../NetworkSettings";
import type { SettingsSectionProps } from "../../types";
import type {
  MCPServerConfig,
  MCPServerStatus,
  MCPToolInfo,
} from "@/types/settings";

const server = (id: string, name: string): MCPServerConfig => ({
  id,
  name,
  command: "npx",
  args: [`@modelcontextprotocol/server-${name}`],
  environment_variables: {},
  enabled: true,
  auto_start: true,
  timeout_seconds: 30,
  max_retries: 3,
});

const tool = (serverId: string, name: string): MCPToolInfo => ({
  server_id: serverId,
  server_name: serverId,
  tool_definition: { name, description: "", input_schema: {} },
  enabled: true,
});

const settingsWith = (
  servers: MCPServerConfig[],
  statuses: Record<string, MCPServerStatus>,
  tools: MCPToolInfo[] = [],
) =>
  ({
    mcpLoading: false,
    mcpServers: servers,
    mcpServerStatuses: statuses,
    mcpTools: tools,
  }) as unknown as SettingsSectionProps["settings"];

/** The row for `name`, found via the server name the row renders. */
const rowFor = (name: string): HTMLElement => {
  const label = screen.getByText(name);
  const row = label.closest("div.border.rounded-lg");
  if (!row) throw new Error(`no server row rendered for '${name}'`);
  return row as HTMLElement;
};

/**
 * LAC-8221. The backend sends serde's externally tagged form of the Rust
 * `MCPServerStatus` enum, pinned by `status_wire_shape_is_externally_tagged`
 * in `mcp_integration.rs`: a unit variant is a bare string, and only
 * `Error(String)` is an object. The section used to read `status.Connected`,
 * which is `undefined` on a string, so a server that was up and serving its
 * tools still rendered an outline "Disconnected" badge.
 *
 * Every status below is written in the real wire shape. Writing them as
 * `{ Connected: null }` is what the old TypeScript type claimed and is the
 * reason this went unnoticed, so a stub in that shape would pass against the
 * bug and prove nothing.
 */
describe("NetworkSettings: MCP server status badges", () => {
  it("renders Connected for a server the backend reports as connected", () => {
    render(
      <NetworkSettings
        settings={settingsWith(
          [server("id-memory", "memory")],
          { "id-memory": "Connected" },
          [tool("id-memory", "create_entities"), tool("id-memory", "read_graph")],
        )}
      />,
    );

    const row = rowFor("memory");
    expect(within(row).getByText("Connected")).toBeInTheDocument();
    expect(within(row).queryByText("Disconnected")).not.toBeInTheDocument();
    // The tools reached the UI; the badge has to agree with them.
    expect(within(row).getByText("2 tools")).toBeInTheDocument();
  });

  it("renders Connecting while the server is starting", () => {
    render(
      <NetworkSettings
        settings={settingsWith([server("id-a", "alpha")], { "id-a": "Connecting" })}
      />,
    );

    expect(within(rowFor("alpha")).getByText("Connecting")).toBeInTheDocument();
  });

  it("renders Timeout when the server timed out", () => {
    render(
      <NetworkSettings
        settings={settingsWith([server("id-a", "alpha")], { "id-a": "Timeout" })}
      />,
    );

    expect(within(rowFor("alpha")).getByText("Timeout")).toBeInTheDocument();
  });

  it("renders Error, with the detail button, for the object-shaped error variant", () => {
    render(
      <NetworkSettings
        settings={settingsWith([server("id-a", "alpha")], {
          "id-a": { Error: "npx: command not found" },
        })}
      />,
    );

    const row = rowFor("alpha");
    expect(within(row).getByText("Error")).toBeInTheDocument();
    // The error path was the one shape that already worked; keep it working.
    expect(within(row).getAllByRole("button").length).toBeGreaterThan(0);
  });

  it("renders Disconnected when the backend reports it, and when the key is absent", () => {
    render(
      <NetworkSettings
        settings={settingsWith(
          [server("id-a", "alpha"), server("id-b", "beta")],
          { "id-a": "Disconnected" },
        )}
      />,
    );

    expect(within(rowFor("alpha")).getByText("Disconnected")).toBeInTheDocument();
    // Negative control: a server with no status entry still reads Disconnected,
    // so the fix is not "everything is Connected now".
    expect(within(rowFor("beta")).getByText("Disconnected")).toBeInTheDocument();
  });
});
