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

export function ToolDebugPanel() {
  const [debugInfo, setDebugInfo] = useState<DebugInfo | null>(null);
  const [registeredTools, setRegisteredTools] = useState<string[] | null>(null);
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
      const result = await invoke("debug_registered_tools");
      setRegisteredTools(result as string[]);
    } catch (err) {
      setError(`Failed to debug registered tools: ${err}`);
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
    (tool) => tool.name === "computer",
  );
  const computerToolMissing = debugInfo && !computerTool;

  return (
    <div className="p-4 bg-gray-900 text-white rounded-lg">
      <h3 className="text-lg font-bold mb-4">Tool Configuration Debug</h3>

      <div className="space-y-4">
        <div className="flex gap-2">
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
            Debug Registered Tools
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
            className={`p-2 rounded ${computerTool.enabled ? "bg-green-900/20 text-green-400" : "bg-red-900/20 text-red-400"}`}
          >
            <strong>Computer Tool Status:</strong>
            <div>Enabled: {computerTool.enabled ? "YES" : "NO"}</div>
            <div>Required: {computerTool.required ? "YES" : "NO"}</div>
            <div>Category: {computerTool.category}</div>
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
              ),
            )}

            <h5 className="font-bold mt-3 mb-1">All Tools:</h5>
            <div className="max-h-40 overflow-y-auto">
              {debugInfo.tools.map((tool) => (
                <div
                  key={tool.name}
                  className={`text-sm ${tool.enabled ? "text-green-400" : "text-red-400"} ${tool.name === "computer" ? "font-bold bg-yellow-900/20" : ""}`}
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
              Registered Tools ({registeredTools.length})
            </h4>
            <div className="max-h-40 overflow-y-auto">
              {registeredTools.map((tool) => (
                <div
                  key={tool}
                  className={`text-sm ${tool === "computer" ? "font-bold text-yellow-400 bg-yellow-900/20" : "text-gray-300"}`}
                >
                  {tool}
                </div>
              ))}
            </div>

            {!registeredTools.includes("computer") && (
              <div className="text-red-400 bg-red-900/20 p-2 rounded mt-2 font-bold">
                🚨 CRITICAL: Computer tool is NOT registered!
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
