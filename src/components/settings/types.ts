import React from "react";
import { useSettingsContext } from "@/contexts/SettingsContext";

export interface SettingsCategory {
  id: string;
  name: string;
  icon: React.ReactNode;
  description: string;
}

export interface ProviderInfo {
  id: string;
  name: string;
  description: string;
  models: string[];
  default_model: string;
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

export interface SettingsSectionProps {
  settings: ReturnType<typeof useSettingsContext>;
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}
