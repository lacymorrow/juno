import type React from "react";
import type { useSettingsContext } from "@/contexts/SettingsContext";

export interface SettingsCategory {
  id: string;
  name: string;
  icon: React.ReactNode;
}

export interface SettingsSectionProps {
  settings: ReturnType<typeof useSettingsContext>;
}

export type {
  ProviderInfo,
  ProviderSettings,
  ToolConfig,
  ToolCategory,
  MCPServerConfig,
  MCPServerStatus,
  MCPToolInfo,
} from "@/types/settings";
