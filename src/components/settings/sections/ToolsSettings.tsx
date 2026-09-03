import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Eye, MousePointer2, RefreshCw, RotateCcw, Shield } from "lucide-react";
import { useState, useEffect } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";

import { SettingsSectionProps } from "../types";

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
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield size={20} />
              Tool Approval
            </CardTitle>
            <CardDescription>
              Control whether the agent requires your approval before executing
              tools
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Require Tool Approval</div>
                <div className="text-sm text-gray-500">
                  Agent will ask for permission before executing each tool
                </div>
              </div>
              <Switch
                checked={toolApprovalRequired}
                disabled={toolApprovalLoading}
                onCheckedChange={handleToggleToolApproval}
              />
            </div>

            {toolApprovalRequired && (
              <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg">
                <div className="flex items-start gap-2">
                  <Shield className="h-4 w-4 text-amber-600 mt-0.5" />
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
            )}
          </CardContent>
        </Card>

        {/* Smooth Mouse Movement Setting */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <MousePointer2 size={20} />
              Smooth Mouse Movement
            </CardTitle>
            <CardDescription>
              Enable or disable smooth mouse movement for computer actions.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Enable Smooth Mouse Movement</div>
                <div className="text-sm text-gray-500">
                  When enabled, mouse movements will be animated for better
                  visual feedback.
                </div>
              </div>
              <Switch
                checked={smoothMouseMovement}
                disabled={smoothMouseMovementLoading}
                onCheckedChange={handleToggleSmoothMouseMovement}
              />
            </div>
          </CardContent>
        </Card>

        {/* Companion Mode */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Eye size={20} />
              Companion Mode
            </CardTitle>
            <CardDescription>
              Watch and advise without taking any actions on your computer
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Enable Companion Mode</div>
                <div className="text-sm text-gray-500">
                  Agent observes your screen and answers questions — no clicking,
                  typing, or automation
                </div>
              </div>
              <Switch
                checked={companionMode}
                disabled={companionModeLoading}
                onCheckedChange={handleToggleCompanionMode}
              />
            </div>

            {companionMode && (
              <div className="p-3 bg-muted border border-border rounded-lg">
                <div className="flex items-start gap-2">
                  <Eye className="h-4 w-4 text-muted-foreground mt-0.5 shrink-0" />
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
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Tool Categories</CardTitle>
            <CardDescription>
              Enable or disable categories of tools available to the AI agent
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.toolConfigLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading tool configurations...</span>
              </div>
            ) : (
              <div className="space-y-4">
                {Object.entries(settings.toolConfigurations).map(
                  ([categoryName, category]) => (
                    <div key={categoryName} className="border rounded-lg">
                      <div className="flex items-center justify-between p-4 border-b">
                        <div>
                          <div className="font-medium">{category.name}</div>
                          <div className="text-sm text-gray-500">
                            {category.description}
                          </div>
                        </div>
                        <Switch
                          checked={category.enabled}
                          onCheckedChange={(enabled) =>
                            handleToggleCategory(categoryName, enabled)
                          }
                        />
                      </div>

                      {category.enabled && (
                        <div className="p-4 space-y-2">
                          {category.tools.map((tool) => (
                            <div
                              key={tool.name}
                              className="flex items-center justify-between p-2 rounded bg-gray-50"
                            >
                              <div>
                                <div className="text-sm font-medium">
                                  {tool.name}
                                </div>
                                {tool.description && (
                                  <div className="text-xs text-gray-500">
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
                      )}
                    </div>
                  )
                )}

                {Object.keys(settings.toolConfigurations).length === 0 && (
                  <div className="text-center py-8 text-gray-500">
                    No tool configurations available
                  </div>
                )}
              </div>
            )}

            <div className="pt-4 border-t">
              <Button
                onClick={handleResetToolConfiguration}
                variant="outline"
                disabled={settings.toolConfigLoading}
                className="w-full"
              >
                <RotateCcw className="w-4 h-4 mr-2" />
                Reset Tool Configuration
              </Button>
            </div>
          </CardContent>
        </Card>
    </div>
  );
}
