import type React from "react";
import type { useSettingsContext } from "@/contexts/SettingsContext";

export interface SettingsCategory {
  id: string;
  name: string;
  icon: React.ReactNode;
  description: string;
  /**
   * Progressive-disclosure tier. `essential` categories are always shown;
   * `advanced` ones stay hidden until the user enables "Advanced settings".
   */
  tier: "essential" | "advanced";
}

export interface SettingsSectionProps {
  settings: ReturnType<typeof useSettingsContext>;
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}

// Domain types live in the canonical src/types/settings.ts — re-export here so
// both `@/types/settings` and `./types` import paths resolve to one definition
// (no divergence). See LAC-2628 type consolidation.
export type {
  ProviderInfo,
  ProviderSettings,
  ToolConfig,
  ToolCategory,
  MCPServerConfig,
  MCPServerStatus,
  MCPToolInfo,
} from "@/types/settings";
