import React from "react";
import { useSettingsContext } from "@/contexts/SettingsContext";

// src/types/settings.ts owns the shapes the backend returns. These were copied
// here once and drifted: the local ProviderInfo had lost model_info,
// is_available, is_default and computer_use_supported, so the settings UI was
// typed against a narrower provider than the one it receives.
export type {
  MCPServerConfig,
  MCPServerStatus,
  MCPToolInfo,
  ProviderInfo,
  ProviderSettings,
  ToolCategory,
  ToolConfig,
} from "@/types/settings";

export interface SettingsCategory {
  id: string;
  name: string;
  icon: React.ReactNode;
  description: string;
  /** Hidden from the sidebar unless the advanced-settings toggle is on. */
  advanced?: boolean;
}

export interface SettingsSectionProps {
  settings: ReturnType<typeof useSettingsContext>;
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}
