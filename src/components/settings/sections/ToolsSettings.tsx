import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Eye, RefreshCw, RotateCcw, Shield } from "lucide-react";
import { useState, useEffect } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";

import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";

export default function ToolsSettings({ settings }: SettingsSectionProps) {
  const [toolApprovalRequired, setToolApprovalRequired] = useState(false);
  const [toolApprovalLoading, setToolApprovalLoading] = useState(false);
  const [smoothMouseMovement, setSmoothMouseMovement] = useState(false);
  const [smoothMouseMovementLoading, setSmoothMouseMovementLoading] =
    useState(false);
  const [companionMode, setCompanionMode] = useState(false);
  const [companionModeLoading, setCompanionModeLoading] = useState(false);

  // Load tool approval setting on mount
  useEffect(() => {
    const loadToolApprovalSetting = async () => {
      try {
        const required = await invoke<boolean>("get_tool_approval_required");
        setToolApprovalRequired(required);
      } catch (error) {
        console.error("Failed to load tool approval setting:", error);
      }
    };
    loadToolApprovalSetting();
  }, []);

  // Load smooth mouse movement setting on mount
  useEffect(() => {
    const loadSmoothMouseMovementSetting = async () => {
      try {
        const enabled = await invoke<boolean>(
          "get_smooth_mouse_movement_setting"
        );
        setSmoothMouseMovement(enabled);
      } catch (error) {
        console.error("Failed to load smooth mouse movement setting:", error);
      }
    };
    loadSmoothMouseMovementSetting();
  }, []);

  // Load companion mode setting on mount
  useEffect(() => {
    invoke<boolean>("get_companion_mode")
      .then(setCompanionMode)
      .catch((error) =>
        console.error("Failed to load companion mode setting:", error)
      );
  }, []);

  const handleToggleToolApproval = async (required: boolean) => {
    setToolApprovalLoading(true);
    try {
      await invoke("set_tool_approval_required", { required });
      setToolApprovalRequired(required);
      toast.success(
        `Tool approval ${required ? "enabled" : "disabled"}${
          required ? " - You will be asked to approve each tool execution" : ""
        }`
      );
    } catch (error) {
      console.error("Failed to toggle tool approval:", error);
      toast.error("Failed to toggle tool approval setting");
    } finally {
      setToolApprovalLoading(false);
    }
  };

  const handleToggleSmoothMouseMovement = async (enabled: boolean) => {
    setSmoothMouseMovementLoading(true);
    try {
      await invoke("set_smooth_mouse_movement_setting", { enabled });
      setSmoothMouseMovement(enabled);
      toast.success(
        `Smooth mouse movement ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle smooth mouse movement:", error);
      toast.error("Failed to toggle smooth mouse movement setting");
    } finally {
      setSmoothMouseMovementLoading(false);
    }
  };

  const handleToggleCompanionMode = async (enabled: boolean) => {
    setCompanionModeLoading(true);
    try {
      await invoke("set_companion_mode", { enabled });
      setCompanionMode(enabled);
      toast.success(
        enabled
          ? "Companion mode enabled — agent will observe without acting"
          : "Companion mode disabled — agent can take actions"
      );
    } catch (error) {
      console.error("Failed to toggle companion mode:", error);
      toast.error("Failed to toggle companion mode");
    } finally {
      setCompanionModeLoading(false);
    }
  };

  const handleToggleCategory = async (
    categoryName: string,
    enabled: boolean
  ) => {
    // Optimistic update - update UI immediately
    const updatedConfigs = { ...settings.toolConfigurations };
    if (updatedConfigs[categoryName]) {
      updatedConfigs[categoryName] = {
        ...updatedConfigs[categoryName],
        enabled,
        // Don't modify individual tool states - the backend only changes category state
        // Individual tools remain unchanged, but their effective state depends on category
      };
    }

    // Update settings state optimistically (this prevents re-render/scroll reset)
    settings.setToolConfigurations(updatedConfigs);

    try {
      // Backend now uses enum format consistently, so categoryName is already correct
      await invoke("set_tool_category_enabled", {
        category: categoryName,
        enabled,
      });
      // Invalidate cache for future loads but don't reload now
      settings.invalidateToolConfigCache();
      toast.success(
        `${categoryName} tools ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle tool category:", error);
      toast.error("Failed to toggle tool category");
      // Revert optimistic update on error
      await settings.loadToolConfigurations();
    }
  };

  const handleToggleTool = async (toolName: string, enabled: boolean) => {
    // Optimistic update - update UI immediately
    const updatedConfigs = { ...settings.toolConfigurations };
    for (const categoryKey in updatedConfigs) {
      const category = updatedConfigs[categoryKey];
      const toolIndex = category.tools.findIndex(
        (tool) => tool.name === toolName
      );
      if (toolIndex !== -1) {
        updatedConfigs[categoryKey] = {
          ...category,
          tools: category.tools.map((tool, index) =>
            index === toolIndex ? { ...tool, enabled } : tool
          ),
        };
        break;
      }
    }

    // Update settings state optimistically (this prevents re-render/scroll reset)
    settings.setToolConfigurations(updatedConfigs);

    try {
      await invoke("set_tool_enabled", { toolName, enabled });
      // Invalidate cache for future loads but don't reload now
      settings.invalidateToolConfigCache();
      toast.success(`${toolName} ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to toggle tool");
      // Revert optimistic update on error
      await settings.loadToolConfigurations();
    }
  };

  const handleResetToolConfiguration = async () => {
    try {
      await invoke("reset_tool_configuration");
      // Force refresh by invalidating cache and reloading
      await settings.invalidateToolConfigCache();
      await settings.loadToolConfigurations();
      toast.success("Tool configuration reset to defaults");
    } catch (error) {
      console.error("Failed to reset tool configuration:", error);
      toast.error("Failed to reset tool configuration");
    }
  };

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Tool Approval"
        footer="Control whether the agent requires your approval before executing tools"
      >
        <SettingsRow
          htmlFor="tool-approval-required"
          label="Require Tool Approval"
          description="Agent will ask for permission before executing each tool"
          below={
            toolApprovalRequired && (
              <div className="rounded-lg border border-amber-200 bg-amber-50 p-3">
                <div className="flex items-start gap-2">
                  <Shield className="mt-0.5 h-4 w-4 text-amber-600" />
                  <div className="text-sm text-amber-800">
                    <div className="font-medium">Approval Required Mode</div>
                    <div className="mt-1">
                      The agent will pause before executing any tool and show
                      you an approval dialog. This provides maximum control but
                      may slow down agent operations.
                    </div>
                  </div>
                </div>
              </div>
            )
          }
        >
          <Switch
            id="tool-approval-required"
            checked={toolApprovalRequired}
            disabled={toolApprovalLoading}
            onCheckedChange={handleToggleToolApproval}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Smooth Mouse Movement"
        footer="Enable or disable smooth mouse movement for computer actions."
      >
        <SettingsRow
          htmlFor="smooth-mouse-movement"
          label="Enable Smooth Mouse Movement"
          description="When enabled, mouse movements will be animated for better visual feedback."
        >
          <Switch
            id="smooth-mouse-movement"
            checked={smoothMouseMovement}
            disabled={smoothMouseMovementLoading}
            onCheckedChange={handleToggleSmoothMouseMovement}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Companion Mode"
        footer="Watch and advise without taking any actions on your computer"
      >
        <SettingsRow
          htmlFor="companion-mode"
          label="Enable Companion Mode"
          description="Agent observes your screen and answers questions — no clicking, typing, or automation"
          below={
            companionMode && (
              <div className="rounded-lg border border-border bg-muted p-3">
                <div className="flex items-start gap-2">
                  <Eye className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
                  <div className="text-sm text-foreground">
                    <div className="font-medium">Companion Mode Active</div>
                    <div className="mt-1">
                      Juno can see your screen and answer questions about it,
                      but will never click, type, or take any automated actions.
                      Useful for guided tours, learning, and pair programming.
                    </div>
                  </div>
                </div>
              </div>
            )
          }
        >
          <Switch
            id="companion-mode"
            checked={companionMode}
            disabled={companionModeLoading}
            onCheckedChange={handleToggleCompanionMode}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Tool Categories"
        footer="Enable or disable categories of tools available to the AI agent"
      >
        {settings.toolConfigLoading ? (
          <SettingsRow
            below={
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading tool configurations...</span>
              </div>
            }
          />
        ) : (
          <>
            {Object.entries(settings.toolConfigurations).map(
              ([categoryName, category]) => (
                <SettingsRow
                  key={categoryName}
                  label={category.name}
                  description={category.description}
                  below={
                    category.enabled && (
                      <div className="space-y-2">
                        {category.tools.map((tool) => (
                          <div
                            key={tool.name}
                            className="flex items-center justify-between gap-4 rounded-md bg-muted p-2"
                          >
                            <div className="min-w-0">
                              <div className="text-[13px] font-medium">
                                {tool.name}
                              </div>
                              {tool.description && (
                                <div className="text-[12px] text-muted-foreground">
                                  {tool.description}
                                </div>
                              )}
                              {tool.required && (
                                <Badge
                                  variant="secondary"
                                  className="mt-1 text-xs"
                                >
                                  Required
                                </Badge>
                              )}
                            </div>
                            <Switch
                              checked={tool.enabled}
                              disabled={tool.required}
                              onCheckedChange={(enabled) =>
                                handleToggleTool(tool.name, enabled)
                              }
                            />
                          </div>
                        ))}
                      </div>
                    )
                  }
                >
                  <Switch
                    checked={category.enabled}
                    onCheckedChange={(enabled) =>
                      handleToggleCategory(categoryName, enabled)
                    }
                  />
                </SettingsRow>
              )
            )}

            {Object.keys(settings.toolConfigurations).length === 0 && (
              <SettingsRow
                below={
                  <div className="py-8 text-center text-muted-foreground">
                    No tool configurations available
                  </div>
                }
              />
            )}
          </>
        )}

        <SettingsRow
          below={
            <Button
              onClick={handleResetToolConfiguration}
              variant="outline"
              disabled={settings.toolConfigLoading}
              className="w-full"
            >
              <RotateCcw className="mr-2 h-4 w-4" />
              Reset Tool Configuration
            </Button>
          }
        />
      </SettingsGroup>
    </div>
  );
}
