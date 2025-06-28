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
import { Input } from "@/components/ui/input";
import { COMMANDS } from "@/lib/constants.generated";

import { SettingsSectionProps } from "../types";

export default function NetworkSettings({ settings }: SettingsSectionProps) {
  const [newServerJson, setNewServerJson] = useState("");
  const [cloudTestPassword, setCloudTestPassword] = useState("");
  const [cloudTestStatus, setCloudTestStatus] = useState("");
  const [isCloudTesting, setIsCloudTesting] = useState(false);

  // JSON coercion function to fix common JSON issues
  const coerceJson = (jsonStr: string): string => {
    try {
      // First try to parse as-is
      JSON.parse(jsonStr);
      return jsonStr;
    } catch (error) {
      // Try to fix common JSON issues
      let fixed = jsonStr.trim();

      // Remove comments
      fixed = fixed.replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");

      // Fix unquoted keys
      fixed = fixed.replace(
        /([{,]\s*)([a-zA-Z_$][a-zA-Z0-9_$-]*)\s*:/g,
        '$1"$2":'
      );

      // Fix single quotes to double quotes
      fixed = fixed.replace(/'/g, '"');

      // Remove trailing commas
      fixed = fixed.replace(/,(\s*[}\]])/g, "$1");

      // Try to complete incomplete JSON
      if (!fixed.startsWith("{") && !fixed.startsWith("[")) {
        fixed = "{" + fixed + "}";
      }

      // Fix missing braces/brackets
      let openBraces = (fixed.match(/\{/g) || []).length;
      let closeBraces = (fixed.match(/\}/g) || []).length;
      while (openBraces > closeBraces) {
        fixed += "}";
        closeBraces++;
      }

      try {
        JSON.parse(fixed);
        toast.info("JSON format was automatically corrected", {
          duration: 3000,
        });
        return fixed;
      } catch (secondError) {
        throw error; // Return original error
      }
    }
  };

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
      const correctedJson = coerceJson(newServerJson);
      const parsedConfig = JSON.parse(correctedJson);

      // Update the textarea with corrected JSON if it was changed
      if (correctedJson !== newServerJson) {
        setNewServerJson(correctedJson);
      }

      // Standard MCP format (Claude Desktop format) where server name is the key
      // Example: { "mcp-server-firecrawl": { "command": "pnpm dlx", "args": ["firecrawl-mcp"], "env": { "FIRECRAWL_API_KEY": "..." } } }
      const serverEntries = Object.entries(parsedConfig);

      if (
        serverEntries.length === 1 &&
        typeof serverEntries[0][1] === "object" &&
        serverEntries[0][1] !== null &&
        "command" in serverEntries[0][1]
      ) {
        const [serverName, serverConfig] = serverEntries[0];
        const config = serverConfig as any;

        const newServer = {
          id: `mcp-${Date.now()}`,
          name: serverName,
          description: config.description || `MCP Server: ${serverName}`,
          command: config.command || "",
          args: config.args || [],
          working_directory: config.working_directory || "",
          environment_variables: config.env || {},
          enabled: true,
          auto_start: config.auto_start || false,
          timeout_seconds: config.timeout_seconds || 30,
          max_retries: config.max_retries || 3,
        };

        await invoke("add_mcp_server", { config: newServer });
        toast.success(`MCP server "${serverName}" added successfully`);
        setNewServerJson("");
        // Backend will emit mcp_state_updated event automatically
        return;
      }

      // If not in standard format, show error
      toast.error(
        "Invalid MCP server configuration format. Please use the standard format with server name as key."
      );
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
      await invoke("toggle_mcp_server", { server_id: serverId, enabled });
      toast.success(`Server ${enabled ? "enabled" : "disabled"}`);
      // Backend will emit mcp_state_updated event automatically
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
      await invoke("toggle_mcp_tool", {
        server_id: serverId,
        tool_name: toolName,
        enabled,
      });
      toast.success(`Tool ${toolName} ${enabled ? "enabled" : "disabled"}`);
      // Backend will emit mcp_state_updated event automatically
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to toggle tool");
    }
  };

  const handleSetCloudPassword = async () => {
    if (!cloudTestPassword.trim()) {
      toast.error("Please enter a password");
      return;
    }

    try {
      await invoke("update_cloud_config", {
        enabled: true,
        api_key: cloudTestPassword,
        device_name: "Juno Test Agent",
        auto_connect: true,
      });
      toast.success("Cloud test password set successfully");
      setCloudTestStatus("Password set - ready for testing");
    } catch (error) {
      console.error("Failed to set cloud password:", error);
      toast.error("Failed to set cloud password");
    }
  };

  const handleTestCloudConnection = async () => {
    setIsCloudTesting(true);
    try {
      const result = await invoke("test_cloud_backend_connection");
      setCloudTestStatus(result as string);
      toast.success("Cloud connection test completed");
    } catch (error) {
      console.error("Cloud test failed:", error);
      setCloudTestStatus(`❌ Test failed: ${error}`);
      toast.error("Cloud connection test failed");
    } finally {
      setIsCloudTesting(false);
    }
  };

  const handleStartCloudConnector = async () => {
    try {
      setIsCloudTesting(true);
      await invoke(COMMANDS.CLOUD_START_PRODUCTION_CLOUD_CONNECTOR);
      setCloudTestStatus("✅ Cloud connector started successfully");
      toast.success("Cloud connector started");
    } catch (error) {
      console.error("Failed to start cloud connector:", error);
      setCloudTestStatus(`❌ Failed to start: ${error}`);
      toast.error("Failed to start cloud connector");
    } finally {
      setIsCloudTesting(false);
    }
  };

  const handleStopCloudConnector = async () => {
    try {
      setIsCloudTesting(true);
      await invoke(COMMANDS.CLOUD_STOP_PRODUCTION_CLOUD_CONNECTOR);
      setCloudTestStatus("🛑 Cloud connector stopped");
      toast.success("Cloud connector stopped");
    } catch (error) {
      console.error("Failed to stop cloud connector:", error);
      setCloudTestStatus(`❌ Failed to stop: ${error}`);
      toast.error("Failed to stop cloud connector");
    } finally {
      setIsCloudTesting(false);
    }
  };

  const handleGetProductionCloudStatus = async () => {
    try {
      const status = await invoke(COMMANDS.CLOUD_GET_PRODUCTION_CLOUD_STATUS);
      setCloudTestStatus(JSON.stringify(status, null, 2));
      toast.success("Production cloud status retrieved");
    } catch (error) {
      console.error("Failed to get production cloud status:", error);
      toast.error("Failed to get production cloud status");
    }
  };

  return (
    <div className="space-y-6">
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
            <Label htmlFor="mcp-json-config">Server Configuration (JSON)</Label>
            <Textarea
              id="mcp-json-config"
              value={newServerJson}
              onChange={(e) => setNewServerJson(e.target.value)}
              placeholder={`{
  "mcp-server-firecrawl": {
    "command": "pnpm dlx",
    "args": ["firecrawl-mcp"],
    "env": {
      "FIRECRAWL_API_KEY": "your-api-key-here"
    }
  }
}`}
              className="h-64 font-mono text-sm"
            />
            <div className="text-xs text-muted-foreground space-y-1">
              <p className="font-medium">
                Standard Format (Claude Desktop compatible):
              </p>
              <p>• Server name as JSON key (e.g. "mcp-server-firecrawl")</p>
              <p>
                • <strong>command</strong>: Executable command (required)
              </p>
              <p>
                • <strong>args</strong>: Command arguments (array)
              </p>
              <p>
                • <strong>env</strong>: Environment variables (object)
              </p>
              <p>
                • <strong>auto_start</strong>: Start automatically on app launch
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

      {/* Cloud Control Testing */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server size={20} />
            Cloud Control Testing
          </CardTitle>
          <CardDescription>
            Set an API key and start the cloud connector to enable remote agent
            control
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="cloud-test-password">
              Cloud Test Password/API Key
            </Label>
            <div className="flex gap-2">
              <Input
                id="cloud-test-password"
                type="password"
                value={cloudTestPassword}
                onChange={(e) => setCloudTestPassword(e.target.value)}
                placeholder="Enter test password or API key"
                className="flex-1"
              />
              <Button
                onClick={handleSetCloudPassword}
                disabled={!cloudTestPassword.trim()}
                className="flex items-center gap-2"
              >
                <Save size={16} />
                Set Password
              </Button>
            </div>
          </div>

          <div className="flex gap-2 flex-wrap">
            <Button
              onClick={handleTestCloudConnection}
              disabled={isCloudTesting}
              className="flex items-center gap-2"
            >
              <RefreshCw
                className={`h-4 w-4 ${isCloudTesting ? "animate-spin" : ""}`}
              />
              Test Health
            </Button>
            <Button
              onClick={handleStartCloudConnector}
              disabled={isCloudTesting}
              className="flex items-center gap-2 bg-green-600 hover:bg-green-700"
            >
              <CheckCircle size={16} />
              Start Connector
            </Button>
            <Button
              onClick={handleStopCloudConnector}
              disabled={isCloudTesting}
              variant="destructive"
              className="flex items-center gap-2"
            >
              <RefreshCw size={16} />
              Stop Connector
            </Button>
            <Button
              onClick={handleGetProductionCloudStatus}
              variant="outline"
              className="flex items-center gap-2"
            >
              <CheckCircle size={16} />
              Get Status
            </Button>
          </div>

          {cloudTestStatus && (
            <div className="space-y-2">
              <Label>Test Status:</Label>
              <div className="p-3 bg-muted/50 rounded font-mono text-sm whitespace-pre-wrap">
                {cloudTestStatus}
              </div>
            </div>
          )}

          <div className="pt-4 border-t text-sm text-muted-foreground">
            <div className="space-y-2">
              <p className="font-medium">How to use cloud control:</p>
              <div className="space-y-2 text-xs">
                <div className="bg-blue-50 p-3 rounded border-l-4 border-blue-400">
                  <p className="font-medium text-blue-800">
                    Step 1: Set API Key
                  </p>
                  <p className="text-blue-700">
                    Enter any password/API key above and click "Set Password"
                  </p>
                </div>
                <div className="bg-green-50 p-3 rounded border-l-4 border-green-400">
                  <p className="font-medium text-green-800">
                    Step 2: Start Connector
                  </p>
                  <p className="text-green-700">
                    Click "Start Connector" to connect your Juno app to the
                    cloud backend
                  </p>
                </div>
                <div className="bg-purple-50 p-3 rounded border-l-4 border-purple-400">
                  <p className="font-medium text-purple-800">
                    Step 3: Send Commands
                  </p>
                  <p className="text-purple-700">
                    Use the WebSocket scripts in <code>/websocket-test/</code>{" "}
                    to send agent commands
                  </p>
                </div>
              </div>
              <p className="text-xs text-amber-600 font-medium">
                💡 Once connected, your Juno agent can be controlled remotely
                via cloud commands!
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
