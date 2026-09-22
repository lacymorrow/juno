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
    /** Drives the computer, but only through a tool version Juno does not
     * send yet. Not a chat-only model — say so accurately. */
  }[];
  is_available: boolean;
  is_default: boolean;
  computer_use_supported: boolean;
  /** Installed, but signed out. Only the Claude CLI can be in this state, and
   * it is the one unavailability fixed in a terminal rather than by pasting a
   * key, so it gets its own words. */
  needs_sign_in: boolean;
}

export interface ProviderSettings {
  id: string;
  api_key?: string;
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

export interface MCPServerStatus {
  Disconnected?: null;
  Connecting?: null;
  Connected?: null;
  Error?: string;
  Timeout?: null;
}

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
