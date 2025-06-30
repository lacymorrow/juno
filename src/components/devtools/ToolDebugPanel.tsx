import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ToolDebugInfo {
  name: string;
  category: string;
  enabled: boolean;
  required: boolean;
  description?: string;
}

interface DebugInfo {
  total_tools: number;
  enabled_tools: number;
  disabled_tools: number;
  tools: ToolDebugInfo[];
  category_states: Record<string, boolean>;
}

interface RegisteredToolsResponse {
  tools: Array<{
    name: string;
    description: string;
    input_schema: any;
    api_type: string;
    beta_flag?: string;
  }>;
  total_count: number;
  note?: string;
}

interface DebugRegisteredResponse {
  total_registered: number;
  tool_names: string[];
  critical_tools: {
    computer: boolean;
    bash: boolean;
    str_replace_based_edit_tool: boolean;
  };
  api_type_counts: Record<string, number>;
  provider_status: string;
  error?: string;
}

export function ToolDebugPanel() {
  const [debugInfo, setDebugInfo] = useState<DebugInfo | null>(null);
  const [registeredTools, setRegisteredTools] =
    useState<RegisteredToolsResponse | null>(null);
  const [detailedDebug, setDetailedDebug] =
    useState<DebugRegisteredResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const debugToolConfiguration = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke("debug_tool_configuration");
      setDebugInfo(result as DebugInfo);
    } catch (err) {
      setError(`Failed to debug tool configuration: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const debugRegisteredTools = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke("get_registered_tools");
      setRegisteredTools(result as RegisteredToolsResponse);
    } catch (err) {
      setError(`Failed to debug registered tools: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const debugDetailedRegistration = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke("debug_registered_tools");
      setDetailedDebug(result as DebugRegisteredResponse);
    } catch (err) {
      setError(`Failed to get detailed debug info: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const resetToolConfig = async () => {
    setLoading(true);
    setError(null);
    try {
      await invoke("debug_reset_tool_config");
      alert("Tool configuration reset successfully!");
      // Refresh the debug info
      await debugToolConfiguration();
    } catch (err) {
      setError(`Failed to reset tool configuration: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const computerTool = debugInfo?.tools.find(
    (tool) => tool.name === "computer"
  );
  const computerToolMissing = debugInfo && !computerTool;

  // Check if computer tool is registered
  const computerToolRegistered = registeredTools?.tools.some(
    (tool) => tool.name === "computer"
  );

  return (
    <div className="p-4 bg-gray-900 text-white rounded-lg">
      <h3 className="text-lg font-bold mb-4">Tool Configuration Debug</h3>

      <div className="space-y-4">
        <div className="flex gap-2 flex-wrap">
          <button
            onClick={debugToolConfiguration}
            disabled={loading}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 rounded"
          >
            Debug Tool Config
          </button>
          <button
            onClick={debugRegisteredTools}
            disabled={loading}
            className="px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-gray-600 rounded"
          >
            Get Registered Tools
          </button>
          <button
            onClick={debugDetailedRegistration}
            disabled={loading}
            className="px-4 py-2 bg-purple-600 hover:bg-purple-700 disabled:bg-gray-600 rounded"
          >
            Detailed Debug
          </button>
          <button
            onClick={resetToolConfig}
            disabled={loading}
            className="px-4 py-2 bg-red-600 hover:bg-red-700 disabled:bg-gray-600 rounded"
          >
            Reset Tool Config
          </button>
        </div>

        {loading && <div className="text-yellow-400">Loading...</div>}
        {error && (
          <div className="text-red-400 bg-red-900/20 p-2 rounded">{error}</div>
        )}

        {computerToolMissing && (
          <div className="text-red-400 bg-red-900/20 p-2 rounded font-bold">
            🚨 CRITICAL: Computer tool is missing from configuration!
          </div>
        )}

        {computerTool && (
          <div
            className={`p-2 rounded ${
              computerTool.enabled
                ? "bg-green-900/20 text-green-400"
                : "bg-red-900/20 text-red-400"
            }`}
          >
            <strong>Computer Tool Status:</strong>
            <div>Enabled: {computerTool.enabled ? "YES" : "NO"}</div>
            <div>Required: {computerTool.required ? "YES" : "NO"}</div>
            <div>Category: {computerTool.category}</div>
          </div>
        )}

        {detailedDebug && (
          <div className="bg-purple-900/20 p-3 rounded border border-purple-600">
            <h4 className="font-bold mb-2 text-purple-300">
              Detailed Registration Debug
            </h4>

            {detailedDebug.error && (
              <div className="text-red-400 bg-red-900/20 p-2 rounded mb-2">
                Error: {detailedDebug.error}
              </div>
            )}

            <div className="mb-2">
              <strong>Provider Status:</strong> {detailedDebug.provider_status}
            </div>
            <div className="mb-2">
              <strong>Total Registered:</strong>{" "}
              {detailedDebug.total_registered}
            </div>

            <div className="mb-3">
              <strong>Critical Tools Status:</strong>
              <div className="ml-4 text-sm">
                <div
                  className={
                    detailedDebug.critical_tools.computer
                      ? "text-green-400"
                      : "text-red-400"
                  }
                >
                  Computer:{" "}
                  {detailedDebug.critical_tools.computer
                    ? "✅ REGISTERED"
                    : "❌ MISSING"}
                </div>
                <div
                  className={
                    detailedDebug.critical_tools.bash
                      ? "text-green-400"
                      : "text-red-400"
                  }
                >
                  Bash:{" "}
                  {detailedDebug.critical_tools.bash
                    ? "✅ REGISTERED"
                    : "❌ MISSING"}
                </div>
                <div
                  className={
                    detailedDebug.critical_tools.str_replace_based_edit_tool
                      ? "text-green-400"
                      : "text-red-400"
                  }
                >
                  String Replace:{" "}
                  {detailedDebug.critical_tools.str_replace_based_edit_tool
                    ? "✅ REGISTERED"
                    : "❌ MISSING"}
                </div>
              </div>
            </div>

            {Object.keys(detailedDebug.api_type_counts).length > 0 && (
              <div className="mb-3">
                <strong>API Type Counts:</strong>
                <div className="ml-4 text-sm">
                  {Object.entries(detailedDebug.api_type_counts).map(
                    ([type, count]) => (
                      <div key={type}>
                        {type}: {count}
                      </div>
                    )
                  )}
                </div>
              </div>
            )}

            <div>
              <strong>All Tool Names:</strong>
              <div className="max-h-32 overflow-y-auto text-sm mt-1">
                {detailedDebug.tool_names.map((name) => (
                  <div
                    key={name}
                    className={`${
                      name === "computer"
                        ? "font-bold text-yellow-400 bg-yellow-900/20"
                        : "text-gray-300"
                    }`}
                  >
                    {name}
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {debugInfo && (
          <div className="bg-gray-800 p-3 rounded">
            <h4 className="font-bold mb-2">Configuration Summary</h4>
            <div>Total Tools: {debugInfo.total_tools}</div>
            <div>Enabled: {debugInfo.enabled_tools}</div>
            <div>Disabled: {debugInfo.disabled_tools}</div>

            <h5 className="font-bold mt-3 mb-1">Category States:</h5>
            {Object.entries(debugInfo.category_states).map(
              ([category, enabled]) => (
                <div
                  key={category}
                  className={enabled ? "text-green-400" : "text-red-400"}
                >
                  {category}: {enabled ? "ENABLED" : "DISABLED"}
                </div>
              )
            )}

            <h5 className="font-bold mt-3 mb-1">All Tools:</h5>
            <div className="max-h-40 overflow-y-auto">
              {debugInfo.tools.map((tool) => (
                <div
                  key={tool.name}
                  className={`text-sm ${
                    tool.enabled ? "text-green-400" : "text-red-400"
                  } ${
                    tool.name === "computer" ? "font-bold bg-yellow-900/20" : ""
                  }`}
                >
                  {tool.name} ({tool.category}) -{" "}
                  {tool.enabled ? "ENABLED" : "DISABLED"}{" "}
                  {tool.required ? "[REQUIRED]" : ""}
                </div>
              ))}
            </div>
          </div>
        )}

        {registeredTools && (
          <div className="bg-gray-800 p-3 rounded">
            <h4 className="font-bold mb-2">
              Registered Tools ({registeredTools.total_count})
            </h4>

            {registeredTools.note && (
              <div className="text-yellow-400 mb-2 text-sm">
                Note: {registeredTools.note}
              </div>
            )}

            <div className="max-h-40 overflow-y-auto">
              {registeredTools.tools.map((tool) => (
                <div
                  key={tool.name}
                  className={`text-sm ${
                    tool.name === "computer"
                      ? "font-bold text-yellow-400 bg-yellow-900/20"
                      : "text-gray-300"
                  }`}
                >
                  <div className="font-medium">{tool.name}</div>
                  <div className="text-xs text-gray-400 ml-2">
                    {tool.description}
                  </div>
                  {tool.beta_flag && (
                    <div className="text-xs text-blue-400 ml-2">
                      Beta: {tool.beta_flag}
                    </div>
                  )}
                </div>
              ))}
            </div>

            {!computerToolRegistered && registeredTools.tools.length > 0 && (
              <div className="text-red-400 bg-red-900/20 p-2 rounded mt-2 font-bold">
                🚨 CRITICAL: Computer tool is NOT registered!
              </div>
            )}

            {computerToolRegistered && (
              <div className="text-green-400 bg-green-900/20 p-2 rounded mt-2">
                ✅ Computer tool is properly registered
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
