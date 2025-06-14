import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { RefreshCw, RotateCcw, Shield } from "lucide-react";
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

  const handleToggleToolApproval = async (required: boolean) => {
    setToolApprovalLoading(true);
    try {
      await invoke("set_tool_approval_required", { required });
      setToolApprovalRequired(required);
      toast.success(
        `Tool approval ${required ? "enabled" : "disabled"}${required ? " - You will be asked to approve each tool execution" : ""}`
      );
    } catch (error) {
      console.error("Failed to toggle tool approval:", error);
      toast.error("Failed to toggle tool approval setting");
    } finally {
      setToolApprovalLoading(false);
    }
  };

  const handleToggleCategory = async (
    categoryName: string,
    enabled: boolean
  ) => {
    try {
      await invoke("set_tool_category_enabled", { categoryName, enabled });
      await settings.loadToolConfigurations();
      toast.success(
        `${categoryName} tools ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle tool category:", error);
      toast.error("Failed to toggle tool category");
    }
  };

  const handleToggleTool = async (toolName: string, enabled: boolean) => {
    try {
      await invoke("set_tool_enabled", { toolName, enabled });
      await settings.loadToolConfigurations();
      toast.success(`${toolName} ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to toggle tool");
    }
  };

  const handleResetToolConfiguration = async () => {
    try {
      await invoke("reset_tool_configuration");
      await settings.loadToolConfigurations();
      toast.success("Tool configuration reset to defaults");
    } catch (error) {
      console.error("Failed to reset tool configuration:", error);
      toast.error("Failed to reset tool configuration");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Tools</h3>

        {/* Tool Approval Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield size={20} />
              Tool Approval
            </CardTitle>
            <CardDescription>
              Control whether the agent requires your approval before executing tools
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
                      The agent will pause before executing any tool and show you an approval dialog. 
                      This provides maximum control but may slow down agent operations.
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
    </div>
  );
}