import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import {
  AlertCircle,
  CheckCircle,
  RefreshCw,
  Server,
  Square,
  Save,
  ExternalLink,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";

import { SettingsSectionProps } from "../types";

export default function NetworkSettings({ settings }: SettingsSectionProps) {
  const [newServerJson, setNewServerJson] = useState("");

  const getMcpServerStatusBadge = (status: any) => {
    if (!status) {
      return <Badge variant="outline">Disconnected</Badge>;
    }

    if (status.Connected !== undefined) {
      return (
        <Badge variant="default" className="bg-green-500">
          Connected
        </Badge>
      );
    } else if (status.Connecting !== undefined) {
      return <Badge variant="secondary">Connecting</Badge>;
    } else if (status.Error !== undefined) {
      return <Badge variant="destructive">Error</Badge>;
    } else if (status.Timeout !== undefined) {
      return <Badge variant="destructive">Timeout</Badge>;
    } else {
      return <Badge variant="outline">Disconnected</Badge>;
    }
  };

  const getMcpServerStatusIcon = (status: any) => {
    if (!status) {
      return <Square className="h-4 w-4 text-gray-400" />;
    }

    if (status.Connected !== undefined) {
      return <CheckCircle className="h-4 w-4 text-green-500" />;
    } else if (status.Connecting !== undefined) {
      return <RefreshCw className="h-4 w-4 text-blue-500 animate-spin" />;
    } else if (status.Error !== undefined || status.Timeout !== undefined) {
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    } else {
      return <Square className="h-4 w-4 text-gray-400" />;
    }
  };

  const handleAddMcpServer = async () => {
    try {
      const parsedEnvVars = JSON.parse(newServerJson);
      const newServer = {
        id: `mcp-${Date.now()}`,
        name: parsedEnvVars.name || "Unnamed Server",
        description: parsedEnvVars.description || "",
        command: parsedEnvVars.command || "",
        args: parsedEnvVars.args || [],
        working_directory: parsedEnvVars.working_directory || "",
        environment_variables: parsedEnvVars.environment_variables || {},
        enabled: true,
        auto_start: parsedEnvVars.auto_start || false,
        timeout_seconds: parsedEnvVars.timeout_seconds || 30,
        max_retries: parsedEnvVars.max_retries || 3,
      };

      await invoke("add_mcp_server", { config: newServer });
      toast.success("MCP server added successfully");
      setNewServerJson("");
      await settings.loadMcpServers();
    } catch (error) {
      console.error("Error adding MCP server:", error);
      if (error instanceof SyntaxError) {
        toast.error("Invalid JSON format");
      } else {
        toast.error("Failed to add MCP server");
      }
    }
  };

  const handleToggleServer = async (serverId: string, enabled: boolean) => {
    try {
      await invoke("toggle_mcp_server", { serverId, enabled });
      await settings.loadMcpServers();
      toast.success(`Server ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle server:", error);
      toast.error("Failed to toggle server");
    }
  };

  const handleToggleTool = async (
    serverId: string,
    toolName: string,
    enabled: boolean
  ) => {
    try {
      await invoke("toggle_mcp_tool", { serverId, toolName, enabled });
      await settings.loadMcpServers();
      toast.success(`Tool ${toolName} ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to toggle tool");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Network</h3>

        {/* MCP Server Configuration - Enhanced JSON Interface */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server size={20} />
              MCP Server Configuration
            </CardTitle>
            <CardDescription>
              Configure Model Context Protocol servers using JSON format
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="mcp-json-config">
                Server Configuration (JSON)
              </Label>
              <Textarea
                id="mcp-json-config"
                value={newServerJson}
                onChange={(e) => setNewServerJson(e.target.value)}
                placeholder={`{
  "name": "File System Server",
  "description": "Access local file system",
  "command": "npx",
  "args": ["@modelcontextprotocol/server-filesystem", "/Users/username/Documents"],
  "working_directory": "",
  "environment_variables": {},
  "auto_start": true,
  "timeout_seconds": 30,
  "max_retries": 3
}`}
                className="h-64 font-mono text-sm"
              />
              <div className="text-xs text-muted-foreground space-y-1">
                <p>
                  • <strong>name</strong>: Display name for the server
                </p>
                <p>
                  • <strong>command</strong>: Executable command (required)
                </p>
                <p>
                  • <strong>args</strong>: Command arguments (array)
                </p>
                <p>
                  • <strong>environment_variables</strong>: Environment
                  variables (object)
                </p>
                <p>
                  • <strong>auto_start</strong>: Start automatically on app
                  launch
                </p>
              </div>
            </div>

            <div className="flex gap-2">
              <Button
                onClick={handleAddMcpServer}
                disabled={!newServerJson.trim() || settings.mcpLoading}
                className="flex items-center gap-2"
              >
                <Save size={16} />
                Add Server
              </Button>
              <Button
                variant="outline"
                onClick={settings.loadMcpServers}
                disabled={settings.mcpLoading}
                className="flex items-center gap-2"
              >
                <RefreshCw
                  className={`h-4 w-4 ${
                    settings.mcpLoading ? "animate-spin" : ""
                  }`}
                />
                Refresh
              </Button>
            </div>

            {/* Common MCP Servers Examples */}
            <div className="pt-4 border-t text-sm text-muted-foreground">
              <div className="space-y-2">
                <p className="font-medium">Common MCP Servers:</p>
                <div className="space-y-1 text-xs font-mono bg-muted/50 p-3 rounded">
                  <div>
                    <strong>File System:</strong> npx
                    @modelcontextprotocol/server-filesystem /path
                  </div>
                  <div>
                    <strong>Everything Server:</strong> npx
                    @modelcontextprotocol/server-everything
                  </div>
                  <div>
                    <strong>Memory:</strong> npx
                    @modelcontextprotocol/server-memory
                  </div>
                </div>
                <a
                  href="https://github.com/modelcontextprotocol/servers"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 text-blue-600 hover:text-blue-700"
                >
                  <ExternalLink className="h-3 w-3" />
                  Browse more servers on GitHub
                </a>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Active MCP Servers */}
        <Card>
          <CardHeader>
            <CardTitle>Active MCP Servers</CardTitle>
            <CardDescription>
              Manage configured MCP servers and their connection status
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.mcpLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading MCP servers...</span>
              </div>
            ) : settings.mcpServers.length > 0 ? (
              <div className="space-y-2">
                {settings.mcpServers.map((server) => {
                  const status = settings.mcpServerStatuses[server.id] || {
                    Disconnected: null,
                  };
                  const hasError = status.Error !== undefined;
                  const serverTools = settings.mcpTools.filter(
                    (tool) => tool.server_id === server.id
                  );

                  return (
                    <div
                      key={server.id}
                      className="flex items-center justify-between p-3 border rounded-lg"
                    >
                      <div className="flex items-center gap-3">
                        {getMcpServerStatusIcon(status)}
                        <div className="flex-1">
                          <div className="font-medium">{server.name}</div>
                          <div className="text-sm text-gray-500">
                            {server.description || server.command}
                          </div>
                          <div className="flex items-center gap-2 mt-1">
                            <Badge variant="outline" className="text-xs">
                              {serverTools.length} tools
                            </Badge>
                            {server.auto_start && (
                              <Badge variant="secondary" className="text-xs">
                                Auto-start
                              </Badge>
                            )}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        {getMcpServerStatusBadge(status)}
                        {hasError && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => {
                              toast.error(`Server Error: ${status.Error}`, {
                                duration: 5000,
                              });
                            }}
                            className="text-red-600 p-1 h-8 w-8"
                          >
                            <AlertCircle className="h-4 w-4" />
                          </Button>
                        )}
                        <Switch
                          checked={server.enabled}
                          onCheckedChange={(enabled) =>
                            handleToggleServer(server.id, enabled)
                          }
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="text-center py-8 text-gray-500">
                <Server className="h-12 w-12 mx-auto mb-4 text-gray-300" />
                <p className="text-lg font-medium mb-2">
                  No MCP servers configured
                </p>
                <p className="text-sm">
                  Add your first MCP server using the configuration above
                </p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Available MCP Tools */}
        {settings.mcpTools.length > 0 && (
          <Card>
            <CardHeader>
              <CardTitle>Available MCP Tools</CardTitle>
              <CardDescription>
                Tools provided by connected MCP servers. Toggle individual tools
                on or off.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                {settings.mcpTools.map((tool) => (
                  <div
                    key={`${tool.server_id}-${tool.tool_definition.name}`}
                    className="flex items-center justify-between p-3 border rounded-lg"
                  >
                    <div className="flex-1">
                      <div className="font-medium">
                        {tool.tool_definition.name}
                      </div>
                      <div className="text-sm text-gray-500 mb-1">
                        from <strong>{tool.server_name}</strong>
                      </div>
                      {tool.tool_definition.description && (
                        <div className="text-xs text-gray-400 max-w-md">
                          {tool.tool_definition.description}
                        </div>
                      )}
                    </div>
                    <Switch
                      checked={tool.enabled}
                      onCheckedChange={(enabled) =>
                        handleToggleTool(
                          tool.server_id,
                          tool.tool_definition.name,
                          enabled
                        )
                      }
                    />
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
