import React from "react";
import { useSettingsManager } from "@/hooks/useSettingsManager";

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
  api_key: string;
  model: string;
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

export interface SettingsFieldProps {
  label: string;
  description?: string;
  children: React.ReactNode;
  error?: string;
  helpText?: string;
}

export interface SettingsSectionProps {
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
}

export interface ShortcutInputProps {
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  error?: string;
}

export interface ProviderFormData {
  name: string;
  apiKey: string;
  model: string;
  maxTokens: number;
  temperature: number;
  systemPrompt: string;
}

export interface ProviderValidationErrors {
  name?: string;
  apiKey?: string;
  model?: string;
  maxTokens?: string;
  temperature?: string;
  systemPrompt?: string;
}

export interface MCPServerData {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export interface SettingsProps {
  settingsManager: ReturnType<typeof useSettingsManager>;
}

// Base settings component props
export interface BaseSettingsProps {
  settingsManager: ReturnType<typeof useSettingsManager>;
  className?: string;
}
