import { useState } from "react";
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
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  AlertCircle,
  CheckCircle,
  RefreshCw,
  Square,
} from "lucide-react";
import { SettingsSectionProps, MCPServerStatus } from "../types";

export default function NetworkSettings({ settings }: SettingsSectionProps) {
  const [newServerJson, setNewServerJson] = useState("");

  const getMcpServerStatusBadge = (status: MCPServerStatus) => {
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

  const getMcpServerStatusIcon = (status: MCPServerStatus) => {
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
      const parsedServer = JSON.parse(newServerJson);
      const newServer = {
        id: `mcp-${Date.now()}`,
        name: parsedServer.name || "Unnamed Server",
        description: parsedServer.description || "",
        command: parsedServer.command || "",
        args: parsedServer.args || [],
        working_directory: parsedServer.working_directory || "",
        environment_variables: parsedServer.environment_variables || {},
        enabled: true,
        auto_start: parsedServer.auto_start || false,
        timeout_seconds: parsedServer.timeout_seconds || 30,
        max_retries: parsedServer.max_retries || 3,
      };

      await invoke("add_mcp_server", { server: newServer });
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

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Network</h3>

        <Card>
          <CardHeader>
            <CardTitle>MCP Servers</CardTitle>
            <CardDescription>
              Manage Model Context Protocol (MCP) servers for external tool
              integration
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.mcpLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading MCP servers...</span>
              </div>
            ) : (
              <div className="space-y-3">
                {settings.mcpServers.map((server) => (
                  <div
                    key={server.id}
                    className="flex items-center justify-between p-3 border rounded-lg"
                  >
                    <div className="flex items-center gap-3">
                      {getMcpServerStatusIcon(
                        settings.mcpServerStatuses[server.id]
                      )}
                      <div>
                        <div className="font-medium">{server.name}</div>
                        <div className="text-sm text-gray-500">
                          {server.description || server.command}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {getMcpServerStatusBadge(
                        settings.mcpServerStatuses[server.id]
                      )}
                      <Switch
                        checked={server.enabled}
                        onCheckedChange={async (enabled) => {
                          try {
                            await invoke("toggle_mcp_server", {
                              serverId: server.id,
                              enabled,
                            });
                            await settings.loadMcpServers();
                          } catch (error) {
                            toast.error("Failed to toggle server");
                          }
                        }}
                      />
                    </div>
                  </div>
                ))}

                {settings.mcpServers.length === 0 && (
                  <div className="text-center py-8 text-gray-500">
                    No MCP servers configured
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Add MCP Server</CardTitle>
            <CardDescription>
              Add a new MCP server configuration in JSON format
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="server-config">Server Configuration (JSON)</Label>
              <Textarea
                id="server-config"
                value={newServerJson}
                onChange={(e) => setNewServerJson(e.target.value)}
                placeholder={JSON.stringify(
                  {
                    name: "Example Server",
                    description: "Description of the server",
                    command: "python",
                    args: ["-m", "example_server"],
                    working_directory: "/path/to/server",
                    environment_variables: {},
                    auto_start: true,
                    timeout_seconds: 30,
                    max_retries: 3,
                  },
                  null,
                  2
                )}
                rows={8}
                className="font-mono text-sm"
              />
            </div>
            <Button
              onClick={handleAddMcpServer}
              disabled={!newServerJson.trim()}
              className="w-full"
            >
              Add Server
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}