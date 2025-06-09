import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { RefreshCw, Terminal } from "lucide-react";
import { SettingsSectionProps, ToolCategory } from "../types";

export default function ToolsSettings({ settings }: SettingsSectionProps) {
  const handleToggleCategory = async (
    categoryName: string,
    enabled: boolean
  ) => {
    try {
      await invoke("set_tool_category_enabled", {
        category: categoryName,
        enabled,
      });
      await settings.loadToolConfigurations();
      toast.success(
        `${categoryName} category ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle category:", error);
      toast.error("Failed to update category setting");
    }
  };

  const handleToggleTool = async (toolName: string, enabled: boolean) => {
    try {
      await invoke("set_tool_enabled", {
        toolName,
        enabled,
      });
      await settings.loadToolConfigurations();
      toast.success(`${toolName} tool ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to update tool setting");
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

        <Card>
          <CardHeader>
            <CardTitle>Tool Configuration</CardTitle>
            <CardDescription>
              Enable or disable specific tools and tool categories for the AI
              agent
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.toolConfigLoading ? (
              <div className="flex items-center gap-2 text-muted-foreground">
                <RefreshCw className="h-4 w-4 animate-spin" />
                Loading tool configurations...
              </div>
            ) : (
              <>
                {Object.entries(settings.toolConfigurations).length === 0 ? (
                  <div className="text-center p-4 text-muted-foreground">
                    <Terminal className="h-8 w-8 mx-auto mb-2 opacity-50" />
                    <p>Tool configuration will be available soon</p>
                    <p className="text-sm">
                      The system is being prepared for tool management
                    </p>
                  </div>
                ) : (
                  <div className="space-y-4">
                    {Object.entries(settings.toolConfigurations).map(
                      ([categoryName, category]: [string, ToolCategory]) => (
                        <div key={categoryName} className="border rounded-lg p-4">
                          <div className="flex items-center justify-between mb-3">
                            <div>
                              <h4 className="font-medium">{categoryName}</h4>
                              <p className="text-sm text-muted-foreground">
                                {category.description}
                              </p>
                            </div>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() =>
                                handleToggleCategory(
                                  categoryName,
                                  !category.enabled
                                )
                              }
                              className="flex items-center gap-2"
                            >
                              {category.enabled ? (
                                <>
                                  <div className="w-4 h-4 bg-green-500 rounded-full"></div>
                                  Enabled
                                </>
                              ) : (
                                <>
                                  <div className="w-4 h-4 bg-gray-400 rounded-full"></div>
                                  Disabled
                                </>
                              )}
                            </Button>
                          </div>

                          {category.tools && category.tools.length > 0 && (
                            <div className="space-y-2">
                              <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                                Tools (
                                {category.tools.filter((t) => t.enabled).length}/
                                {category.tools.length})
                              </div>
                              <div className="grid gap-2">
                                {category.tools.map((tool) => (
                                  <div
                                    key={tool.name}
                                    className="flex items-center justify-between py-2 px-3 bg-gray-50 rounded"
                                  >
                                    <div className="flex-1">
                                      <div className="flex items-center gap-2">
                                        <span className="text-sm font-medium">
                                          {tool.name}
                                        </span>
                                        {tool.required && (
                                          <Badge
                                            variant="secondary" 
                                            className="text-xs"
                                          >
                                            Required
                                          </Badge>
                                        )}
                                      </div>
                                      {tool.description && (
                                        <p className="text-xs text-muted-foreground mt-1">
                                          {tool.description}
                                        </p>
                                      )}
                                    </div>
                                    <Button
                                      variant="ghost"
                                      size="sm"
                                      onClick={() =>
                                        handleToggleTool(tool.name, !tool.enabled)
                                      }
                                      disabled={tool.required && tool.enabled}
                                      className="ml-2"
                                    >
                                      {tool.enabled ? (
                                        <div className="w-3 h-3 bg-green-500 rounded-full"></div>
                                      ) : (
                                        <div className="w-3 h-3 bg-gray-400 rounded-full"></div>
                                      )}
                                    </Button>
                                  </div>
                                ))}
                              </div>
                            </div>
                          )}
                        </div>
                      )
                    )}

                    <div className="flex gap-2 pt-4">
                      <Button
                        variant="outline"
                        onClick={handleResetToolConfiguration}
                        className="flex items-center gap-2"
                        disabled={settings.toolConfigLoading}
                      >
                        <RefreshCw size={16} />
                        Reset to Defaults
                      </Button>
                      <Button
                        variant="outline"
                        onClick={settings.loadToolConfigurations}
                        className="flex items-center gap-2"
                        disabled={settings.toolConfigLoading}
                      >
                        <RefreshCw size={16} />
                        Refresh
                      </Button>
                    </div>
                  </div>
                )}
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}