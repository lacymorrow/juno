// Shared types for settings across the application
export interface ProviderInfo {
  id: string;
  name: string;
  description: string;
  models: string[];
  default_model: string;
  model_info: {
    id: string;
    name: string;
    supports_computer_use: boolean;
    is_recommended: boolean;
    /** The provider lists this model as legacy. Hidden unless advanced
     * settings are on, or it is the model currently in use. */
    is_legacy: boolean;
	/** Model drives the computer only through computer_toolset_20260801 (advanced). */
	requires_computer_toolset?: boolean;
    /** Drives the computer, but only through a tool version Juno does not
     * send yet. Not a chat-only model — say so accurately. */
  }[];
  is_available: boolean;
  is_default: boolean;
  computer_use_supported: boolean;
  /** Installed, but signed out: a CLI (Claude, Codex) or GitHub Copilot with
   * no usable GitHub sign-in. Fixed by signing in rather than by pasting a
   * key, so it gets its own words. */
  needs_sign_in: boolean;
  /** A key of this provider's own is saved in the credential store. The key
   * itself never reaches the page. */
  has_saved_key: boolean;
}

export interface ProviderSettings {
  id: string;
  model?: string;
  max_tokens?: number;
  temperature?: number;
  system_prompt?: string;
  /**
   * How hard the model thinks per turn, where the provider exposes it.
   * Hidden advanced setting: the backend owns it and no UI edits it. Listed
   * here only so the type matches the Rust `ProviderConfig` it comes from.
   */
  effort?: string;
  /**
   * Load the MCP servers on the person's own Claude account (claude.ai
   * connectors, user-level servers). Claude CLI provider only; on by
   * default. Off runs the CLI with --strict-mcp-config (LAC-4056).
   */
  load_account_mcp?: boolean;
}

export interface ToolConfig {
  name: string;
  category: string;
  enabled: boolean;
  description?: string;
  required: boolean;
}

export interface ToolCategory {
  name: string;
  description: string;
  enabled: boolean;
  tools: ToolConfig[];
}

export interface MCPServerConfig {
  id: string;
  name: string;
  description?: string;
  command: string;
  args: string[];
  working_directory?: string;
  environment_variables: Record<string, string>;
  enabled: boolean;
  auto_start: boolean;
  timeout_seconds: number;
  max_retries: number;
}

/**
 * What the backend's `MCPServerStatus` enum looks like on the wire.
 *
 * It is externally tagged (serde's default), so a unit variant arrives as a
 * bare string and only `Error(String)` arrives as an object. This used to be
 * declared as one object with five optional keys, which made
 * `status.Connected` read `undefined` for every connected server and drew a
 * "Disconnected" badge over a server that was up (LAC-8221). The Rust test
 * `status_wire_shape_is_externally_tagged` pins the shape; keep the two in
 * step.
 */
export type MCPServerStatus =
  | "Disconnected"
  | "Connecting"
  | "Connected"
  | "Timeout"
  | { Error: string };

export interface MCPToolInfo {
  server_id: string;
  server_name: string;
  tool_definition: {
    name: string;
    description: string;
    input_schema: any;
  };
  enabled: boolean;
}

export interface AppPermissionStatus {
  permission_type: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

export interface PermissionsState {
  accessibility: AppPermissionStatus;
  screen_recording: AppPermissionStatus;
  microphone: AppPermissionStatus;
  input_monitoring: AppPermissionStatus;
  /** Accessibility and Screen Recording only. */
  all_granted: boolean;
  /** Every permission, the optional two included. */
  everything_granted?: boolean;
  app_name: string;
}
