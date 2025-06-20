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
  }[];
  is_available: boolean;
  is_default: boolean;
  computer_use_supported: boolean;
}

export interface ProviderSettings {
  id: string;
  api_key?: string;
  model?: string;
  max_tokens?: number;
  temperature?: number;
  system_prompt?: string;
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

export interface PermissionsState {
  accessibility: { granted: boolean; required: boolean };
  screenRecording: { granted: boolean; required: boolean };
  microphone: { granted: boolean; required: boolean };
  allGranted: boolean;
  appName: string;
}
