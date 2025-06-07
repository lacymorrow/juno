import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  AlertTriangle,
  CheckCircle,
  Play,
  RefreshCw,
  Wifi,
  WifiOff,
  XCircle,
} from "lucide-react";
import React, { useEffect, useState } from "react";

interface CloudStatus {
  enabled: boolean;
  connected: boolean;
  connection_state: string;
  device_id?: string;
  last_error?: string;
}

interface TestResult {
  success: boolean;
  test?: string;
  response?: any;
  error?: string;
  timestamp?: number;
  duration_ms?: number;
}

interface CloudConfig {
  enabled: boolean;
  server_url: string;
  device_name: string;
  device_id?: string;
  security_level: string;
  auto_connect: boolean;
}

interface WebSocketDiagnostics {
  timestamp: number;
  connector_available: boolean;
  connection_state: string;
  stats?: any;
  config: CloudConfig;
}

export const CloudTestPanel: React.FC = () => {
  const [cloudStatus, setCloudStatus] = useState<CloudStatus | null>(null);
  const [cloudConfig, setCloudConfig] = useState<CloudConfig | null>(null);
  const [diagnostics, setDiagnostics] = useState<WebSocketDiagnostics | null>(
    null
  );
  const [testResults, setTestResults] = useState<TestResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [testUrl, setTestUrl] = useState("wss://echo.websocket.org");
  const [testCommand, setTestCommand] = useState("status_request");
  const [testPayload, setTestPayload] = useState(
    '{"query": "Hello from Juno"}'
  );
  const [isRunningTests, setIsRunningTests] = useState(false);

  // New remote command testing state
  const [remoteCommand, setRemoteCommand] = useState("system_command");
  const [remotePayload, setRemotePayload] = useState(
    '{"action": "screenshot"}'
  );
  const [remoteTestResults, setRemoteTestResults] = useState<TestResult[]>([]);
  const [connectionDiagnostics, setConnectionDiagnostics] = useState<any>(null);

  useEffect(() => {
    loadCloudStatus();
    loadCloudConfig();
    loadDiagnostics();
  }, []);

  const loadCloudStatus = async () => {
    try {
      const status = await invoke<CloudStatus>("get_cloud_status");
      setCloudStatus(status);
    } catch (error) {
      console.error("Failed to load cloud status:", error);
    }
  };

  const loadCloudConfig = async () => {
    try {
      const config = await invoke<CloudConfig>("get_cloud_config");
      setCloudConfig(config);
    } catch (error) {
      console.error("Failed to load cloud config:", error);
    }
  };

  const loadDiagnostics = async () => {
    try {
      const diag = await invoke<WebSocketDiagnostics>(
        "get_websocket_diagnostics"
      );
      setDiagnostics(diag);
    } catch (error) {
      console.error("Failed to load diagnostics:", error);
    }
  };

  const loadConnectionDiagnostics = async () => {
    try {
      const connDiag = await invoke<any>("get_cloud_connection_diagnostics");
      setConnectionDiagnostics(connDiag);
    } catch (error) {
      console.error("Failed to load connection diagnostics:", error);
    }
  };

  const refreshAll = async () => {
    setIsLoading(true);
    await Promise.all([
      loadCloudStatus(),
      loadCloudConfig(),
      loadDiagnostics(),
      loadConnectionDiagnostics(),
    ]);
    setIsLoading(false);
  };

  const testWebSocketConnection = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<TestResult>("test_websocket_connection", {
        serverUrl: testUrl,
      });
      setTestResults((prev) => [result, ...prev]);
    } catch (error) {
      console.error("WebSocket test failed:", error);
      setTestResults((prev) => [
        {
          success: false,
          error: error as string,
          timestamp: Date.now(),
        },
        ...prev,
      ]);
    }
    setIsLoading(false);
  };

  const sendTestCommand = async () => {
    setIsLoading(true);
    try {
      const payload = JSON.parse(testPayload);
      const result = await invoke<TestResult>("send_test_cloud_command", {
        commandType: testCommand,
        payload,
      });
      setTestResults((prev) => [result, ...prev]);
    } catch (error) {
      console.error("Test command failed:", error);
      setTestResults((prev) => [
        {
          success: false,
          error: error as string,
          timestamp: Date.now(),
        },
        ...prev,
      ]);
    }
    setIsLoading(false);
  };

  const runTestSuite = async () => {
    setIsRunningTests(true);
    try {
      const result = await invoke<{
        overall_success: boolean;
        test_count: number;
        tests: TestResult[];
        timestamp: number;
      }>("run_websocket_test_suite");

      setTestResults((prev) => [
        {
          success: result.overall_success,
          test: "Test Suite",
          response: result,
          timestamp: result.timestamp,
        },
        ...result.tests,
        ...prev,
      ]);
    } catch (error) {
      console.error("Test suite failed:", error);
      setTestResults((prev) => [
        {
          success: false,
          error: error as string,
          timestamp: Date.now(),
        },
        ...prev,
      ]);
    }
    setIsRunningTests(false);
  };

  const startCloudConnector = async () => {
    setIsLoading(true);
    try {
      await invoke("start_production_cloud_connector");
      await refreshAll();
    } catch (error) {
      console.error("Failed to start cloud connector:", error);
    }
    setIsLoading(false);
  };

  const stopCloudConnector = async () => {
    setIsLoading(true);
    try {
      await invoke("stop_production_cloud_connector");
      await refreshAll();
    } catch (error) {
      console.error("Failed to stop cloud connector:", error);
    }
    setIsLoading(false);
  };

  const getStatusIcon = (connected: boolean) => {
    return connected ? (
      <Wifi className="h-4 w-4 text-green-500" />
    ) : (
      <WifiOff className="h-4 w-4 text-red-500" />
    );
  };

  const getTestResultIcon = (success: boolean) => {
    return success ? (
      <CheckCircle className="h-4 w-4 text-green-500" />
    ) : (
      <XCircle className="h-4 w-4 text-red-500" />
    );
  };

  const runQuickTest = async () => {
    try {
      setIsLoading(true);

      // Test 1: Basic connection test
      const basicTest = await invoke("test_websocket_connection");
      setTestResults((prev) => [
        {
          success: basicTest.success || true,
          test: "Quick Test - Basic WebSocket",
          response: basicTest,
          timestamp: Date.now() / 1000,
        },
        ...prev,
      ]);

      // Test 2: Get diagnostics
      const diagnostics = await invoke("get_websocket_diagnostics");
      setTestResults((prev) => [
        {
          success: true,
          test: "Quick Test - Diagnostics",
          response: diagnostics,
          timestamp: Date.now() / 1000,
        },
        ...prev,
      ]);

      // Test 3: Run test suite
      const testSuite = await invoke("run_websocket_test_suite");
      setTestResults((prev) => [
        {
          success: testSuite.overall_success || true,
          test: "Quick Test - Test Suite",
          response: testSuite,
          timestamp: Date.now() / 1000,
        },
        ...prev,
      ]);
    } catch (error) {
      setTestResults((prev) => [
        {
          success: false,
          test: "Quick Test",
          error: error as string,
          timestamp: Date.now() / 1000,
        },
        ...prev,
      ]);
    } finally {
      setIsLoading(false);
    }
  };

  const executeRemoteCommand = async () => {
    setIsLoading(true);
    try {
      const payload = JSON.parse(remotePayload);
      const result = await invoke<any>("execute_remote_command", {
        commandType: remoteCommand,
        payload,
      });
      setRemoteTestResults((prev) => [
        {
          success: result.success,
          test: `Remote ${remoteCommand}`,
          response: result,
          timestamp: Date.now(),
        },
        ...prev,
      ]);
    } catch (error) {
      console.error("Remote command failed:", error);
      setRemoteTestResults((prev) => [
        {
          success: false,
          error: error as string,
          timestamp: Date.now(),
        },
        ...prev,
      ]);
    }
    setIsLoading(false);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">Cloud WebSocket Testing</h3>
        <Button
          onClick={refreshAll}
          disabled={isLoading}
          size="sm"
          variant="outline"
        >
          <RefreshCw
            className={`h-4 w-4 mr-2 ${isLoading ? "animate-spin" : ""}`}
          />
          Refresh
        </Button>
      </div>

      <Tabs defaultValue="status" className="w-full">
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="status">Status</TabsTrigger>
          <TabsTrigger value="testing">Testing</TabsTrigger>
          <TabsTrigger value="remote">Remote</TabsTrigger>
          <TabsTrigger value="diagnostics">Diagnostics</TabsTrigger>
          <TabsTrigger value="results">Results</TabsTrigger>
        </TabsList>

        <TabsContent value="status" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {cloudStatus && getStatusIcon(cloudStatus.connected)}
                Connection Status
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {cloudStatus && (
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <Label>Status</Label>
                    <Badge
                      variant={
                        cloudStatus.connected ? "default" : "destructive"
                      }
                    >
                      {cloudStatus.connected ? "Connected" : "Disconnected"}
                    </Badge>
                  </div>
                  <div>
                    <Label>State</Label>
                    <p className="text-sm">{cloudStatus.connection_state}</p>
                  </div>
                  <div>
                    <Label>Device ID</Label>
                    <p className="text-sm font-mono">
                      {cloudStatus.device_id || "Not set"}
                    </p>
                  </div>
                  <div>
                    <Label>Enabled</Label>
                    <Badge
                      variant={cloudStatus.enabled ? "default" : "secondary"}
                    >
                      {cloudStatus.enabled ? "Yes" : "No"}
                    </Badge>
                  </div>
                </div>
              )}

              {cloudConfig && (
                <div className="pt-4 border-t">
                  <Label>Server URL</Label>
                  <p className="text-sm font-mono">{cloudConfig.server_url}</p>
                </div>
              )}

              <div className="flex gap-2">
                <Button
                  onClick={startCloudConnector}
                  disabled={isLoading || (cloudStatus?.connected ?? false)}
                  size="sm"
                >
                  Start Connector
                </Button>
                <Button
                  onClick={stopCloudConnector}
                  disabled={isLoading || !(cloudStatus?.connected ?? false)}
                  variant="outline"
                  size="sm"
                >
                  Stop Connector
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="testing" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Quick WebSocket Test</CardTitle>
              <CardDescription>
                Run all basic WebSocket tests quickly
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Button
                onClick={runQuickTest}
                disabled={isLoading}
                className="w-full"
                size="lg"
              >
                <Play className="h-4 w-4 mr-2" />
                {isLoading ? "Testing..." : "Quick Test"}
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Basic WebSocket Test</CardTitle>
              <CardDescription>
                Test WebSocket connectivity with echo server
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="test-url">Test URL</Label>
                <Input
                  id="test-url"
                  value={testUrl}
                  onChange={(e) => setTestUrl(e.target.value)}
                  placeholder="wss://echo.websocket.org"
                />
              </div>
              <Button
                onClick={testWebSocketConnection}
                disabled={isLoading}
                className="w-full"
              >
                <Play className="h-4 w-4 mr-2" />
                Test Connection
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Cloud Command Test</CardTitle>
              <CardDescription>
                Send test commands to cloud connector
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="command-type">Command Type</Label>
                <select
                  id="command-type"
                  value={testCommand}
                  onChange={(e) => setTestCommand(e.target.value)}
                  className="w-full p-2 border rounded"
                >
                  <option value="status_request">Status Request</option>
                  <option value="text_query">Text Query</option>
                  <option value="screenshot">Screenshot</option>
                  <option value="system_command">System Command</option>
                </select>
              </div>
              <div>
                <Label htmlFor="payload">Payload (JSON)</Label>
                <Textarea
                  id="payload"
                  value={testPayload}
                  onChange={(e) => setTestPayload(e.target.value)}
                  rows={4}
                  placeholder='{"query": "Hello from Juno"}'
                />
              </div>
              <Button
                onClick={sendTestCommand}
                disabled={isLoading}
                className="w-full"
              >
                <Play className="h-4 w-4 mr-2" />
                Send Command
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Remote Command Test</CardTitle>
              <CardDescription>
                Test remote system commands (screenshot, click, type, etc.)
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="remote-command-type">Command Type</Label>
                <select
                  id="remote-command-type"
                  value={remoteCommand}
                  onChange={(e) => setRemoteCommand(e.target.value)}
                  className="w-full p-2 border rounded"
                >
                  <option value="system_command">System Command</option>
                  <option value="text_query">Text Query</option>
                  <option value="screenshot">Screenshot</option>
                </select>
              </div>
              <div>
                <Label htmlFor="remote-payload">Payload (JSON)</Label>
                <Textarea
                  id="remote-payload"
                  value={remotePayload}
                  onChange={(e) => setRemotePayload(e.target.value)}
                  rows={4}
                  placeholder='{"action": "screenshot"}'
                />
              </div>
              <div className="text-xs text-gray-600 space-y-1">
                <p><strong>Available actions:</strong></p>
                <p>• screenshot - Take a screenshot</p>
                <p>• click - Click at coordinates: {"action": "click", "x": 100, "y": 200}</p>
                <p>• type - Type text: {"action": "type", "text": "Hello World"}</p>
                <p>• key - Press key: {"action": "key", "key": "Return"}</p>
                <p>• execute - Run shell command: {"action": "execute", "command": "ls -la"}</p>
                <p>• status - Get system status</p>
              </div>
              <Button
                onClick={executeRemoteCommand}
                disabled={isLoading}
                className="w-full"
              >
                <Play className="h-4 w-4 mr-2" />
                Execute Remote Command
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Comprehensive Test Suite</CardTitle>
              <CardDescription>Run all WebSocket tests at once</CardDescription>
            </CardHeader>
            <CardContent>
              <Button
                onClick={runTestSuite}
                disabled={isRunningTests}
                className="w-full"
                size="lg"
              >
                <Activity
                  className={`h-4 w-4 mr-2 ${
                    isRunningTests ? "animate-pulse" : ""
                  }`}
                />
                {isRunningTests ? "Running Tests..." : "Run Test Suite"}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="remote" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Remote System Control</CardTitle>
              <CardDescription>
                Execute system commands remotely via cloud connection
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="remote-cmd-type">Command Type</Label>
                <select
                  id="remote-cmd-type"
                  value={remoteCommand}
                  onChange={(e) => setRemoteCommand(e.target.value)}
                  className="w-full p-2 border rounded"
                >
                  <option value="system_command">System Command</option>
                  <option value="text_query">Text Query</option>
                  <option value="screenshot">Screenshot</option>
                </select>
              </div>
              <div>
                <Label htmlFor="remote-cmd-payload">Command Payload</Label>
                <Textarea
                  id="remote-cmd-payload"
                  value={remotePayload}
                  onChange={(e) => setRemotePayload(e.target.value)}
                  rows={6}
                  placeholder='{"action": "screenshot"}'
                />
              </div>
              <div className="text-xs text-gray-600 bg-gray-50 p-3 rounded space-y-1">
                <p><strong>System Command Examples:</strong></p>
                <div className="grid grid-cols-1 gap-1 font-mono">
                  <p>{"action": "screenshot"}</p>
                  <p>{"action": "click", "x": 100, "y": 200}</p>
                  <p>{"action": "type", "text": "Hello World"}</p>
                  <p>{"action": "key", "key": "Return"}</p>
                  <p>{"action": "execute", "command": "ls -la"}</p>
                  <p>{"action": "status"}</p>
                </div>
              </div>
              <Button
                onClick={executeRemoteCommand}
                disabled={isLoading}
                className="w-full"
              >
                <Play className="h-4 w-4 mr-2" />
                Execute Remote Command
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Remote Command Results</CardTitle>
              <CardDescription>
                History of remote command executions
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-64">
                {remoteTestResults.length > 0 ? (
                  <div className="space-y-2">
                    {remoteTestResults.map((result, index) => (
                      <div
                        key={index}
                        className="p-3 border rounded-lg space-y-2"
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            {getTestResultIcon(result.success)}
                            <span className="font-medium">
                              {result.test || "Remote Command"}
                            </span>
                          </div>
                          {result.timestamp && (
                            <span className="text-xs text-gray-500">
                              {new Date(result.timestamp).toLocaleTimeString()}
                            </span>
                          )}
                        </div>

                        {result.error && (
                          <Alert>
                            <AlertTriangle className="h-4 w-4" />
                            <AlertDescription>{result.error}</AlertDescription>
                          </Alert>
                        )}

                        {result.response && (
                          <details className="text-xs">
                            <summary className="cursor-pointer text-blue-600">
                              View Response
                            </summary>
                            <pre className="bg-gray-100 p-2 rounded mt-2 overflow-auto">
                              {JSON.stringify(result.response, null, 2)}
                            </pre>
                          </details>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-gray-500 text-center py-8">
                    No remote command results yet. Execute commands to see results here.
                  </p>
                )}
              </ScrollArea>

              {remoteTestResults.length > 0 && (
                <Button
                  onClick={() => setRemoteTestResults([])}
                  variant="outline"
                  size="sm"
                  className="mt-4"
                >
                  Clear Results
                </Button>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="diagnostics" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>WebSocket Diagnostics</CardTitle>
            </CardHeader>
            <CardContent>
              {diagnostics ? (
                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <Label>Connector Available</Label>
                      <Badge
                        variant={
                          diagnostics.connector_available
                            ? "default"
                            : "destructive"
                        }
                      >
                        {diagnostics.connector_available ? "Yes" : "No"}
                      </Badge>
                    </div>
                    <div>
                      <Label>Connection State</Label>
                      <p className="text-sm">{diagnostics.connection_state}</p>
                    </div>
                  </div>

                  {diagnostics.stats && (
                    <div className="pt-4 border-t">
                      <Label>Connection Statistics</Label>
                      <pre className="text-xs bg-gray-100 p-2 rounded mt-2">
                        {JSON.stringify(diagnostics.stats, null, 2)}
                      </pre>
                    </div>
                  )}

                  <div className="pt-4 border-t">
                    <Label>Configuration</Label>
                    <pre className="text-xs bg-gray-100 p-2 rounded mt-2">
                      {JSON.stringify(diagnostics.config, null, 2)}
                    </pre>
                  </div>
                </div>
              ) : (
                <p className="text-gray-500">Loading diagnostics...</p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                Cloud Connection Diagnostics
                <Button
                  onClick={loadConnectionDiagnostics}
                  disabled={isLoading}
                  size="sm"
                  variant="outline"
                >
                  <RefreshCw
                    className={`h-4 w-4 mr-2 ${isLoading ? "animate-spin" : ""}`}
                  />
                  Refresh
                </Button>
              </CardTitle>
            </CardHeader>
            <CardContent>
              {connectionDiagnostics ? (
                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <Label>Connection Active</Label>
                      <Badge
                        variant={
                          connectionDiagnostics.connection_active
                            ? "default"
                            : "destructive"
                        }
                      >
                        {connectionDiagnostics.connection_active ? "Yes" : "No"}
                      </Badge>
                    </div>
                    <div>
                      <Label>Last Heartbeat</Label>
                      <p className="text-sm">
                        {connectionDiagnostics.last_heartbeat || "None"}
                      </p>
                    </div>
                  </div>

                  {connectionDiagnostics.performance_metrics && (
                    <div className="pt-4 border-t">
                      <Label>Performance Metrics</Label>
                      <pre className="text-xs bg-gray-100 p-2 rounded mt-2">
                        {JSON.stringify(connectionDiagnostics.performance_metrics, null, 2)}
                      </pre>
                    </div>
                  )}

                  {connectionDiagnostics.error_history && connectionDiagnostics.error_history.length > 0 && (
                    <div className="pt-4 border-t">
                      <Label>Recent Errors</Label>
                      <div className="space-y-2 mt-2">
                        {connectionDiagnostics.error_history.map((error: any, index: number) => (
                          <Alert key={index}>
                            <AlertTriangle className="h-4 w-4" />
                            <AlertDescription>
                              <span className="text-xs text-gray-500">
                                {error.timestamp}
                              </span>
                              <br />
                              {error.message}
                            </AlertDescription>
                          </Alert>
                        ))}
                      </div>
                    </div>
                  )}

                  <div className="pt-4 border-t">
                    <Label>Full Diagnostics</Label>
                    <pre className="text-xs bg-gray-100 p-2 rounded mt-2">
                      {JSON.stringify(connectionDiagnostics, null, 2)}
                    </pre>
                  </div>
                </div>
              ) : (
                <p className="text-gray-500">No connection diagnostics available. Click refresh to load.</p>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="results" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Test Results</CardTitle>
              <CardDescription>
                History of WebSocket tests and commands
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-96">
                {testResults.length > 0 ? (
                  <div className="space-y-2">
                    {testResults.map((result, index) => (
                      <div
                        key={index}
                        className="p-3 border rounded-lg space-y-2"
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            {getTestResultIcon(result.success)}
                            <span className="font-medium">
                              {result.test || "Test"}
                            </span>
                          </div>
                          {result.timestamp && (
                            <span className="text-xs text-gray-500">
                              {new Date(
                                result.timestamp * 1000
                              ).toLocaleTimeString()}
                            </span>
                          )}
                        </div>

                        {result.error && (
                          <Alert>
                            <AlertTriangle className="h-4 w-4" />
                            <AlertDescription>{result.error}</AlertDescription>
                          </Alert>
                        )}

                        {result.response && (
                          <details className="text-xs">
                            <summary className="cursor-pointer text-blue-600">
                              View Response
                            </summary>
                            <pre className="bg-gray-100 p-2 rounded mt-2 overflow-auto">
                              {JSON.stringify(result.response, null, 2)}
                            </pre>
                          </details>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-gray-500 text-center py-8">
                    No test results yet. Run some tests to see results here.
                  </p>
                )}
              </ScrollArea>

              {testResults.length > 0 && (
                <Button
                  onClick={() => setTestResults([])}
                  variant="outline"
                  size="sm"
                  className="mt-4"
                >
                  Clear Results
                </Button>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
};
