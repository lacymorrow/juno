/**
 * Settings Demo Component
 *
 * Demonstrates the new centralized, reactive SettingsManager system.
 * Shows how all settings are now unified and automatically reactive
 * across all components.
 */

import React, { useState, useEffect } from "react";
import { useSettingsManager } from "../hooks/useSettingsManager";

interface SettingsDemoProps {
  onClose?: () => void;
}

export const SettingsDemo: React.FC<SettingsDemoProps> = ({ onClose }) => {
  const {
    settings,
    loading,
    error,
    updateKeyboardShortcuts,
    updateFloatingBar,
    updateAgent,
    updateCloud,
    updateAudio,
    updateUI,
    resetSection,
    resetAll,
    migrateLegacy,
  } = useSettingsManager();

  const [activeTab, setActiveTab] = useState<string>("keyboard");
  const [migrationStatus, setMigrationStatus] = useState<string>("");

  // Handle migration from legacy settings
  const handleMigration = async () => {
    setMigrationStatus("Migrating legacy settings...");
    try {
      await migrateLegacy();
      setMigrationStatus("✅ Migration completed successfully!");
      setTimeout(() => setMigrationStatus(""), 3000);
    } catch (err) {
      setMigrationStatus(`❌ Migration failed: ${err}`);
      setTimeout(() => setMigrationStatus(""), 5000);
    }
  };

  // Demonstrate keyboard shortcuts updates
  const updateShortcuts = async () => {
    await updateKeyboardShortcuts({
      agent_mode_toggle: "Option+A",
      dictation_input: "Option+D",
      stop_current_action: "Escape",
      floating_bar_toggle: "Option+F",
      screenshot_mode: "Option+S",
      quick_command: "Option+Q",
    });
  };

  // Demonstrate floating bar updates
  const updateFloatingBarSettings = async () => {
    await updateFloatingBar({
      enabled: !settings?.floating_bar.enabled,
      position: { x: 100, y: 100 },
      size: { width: 200, height: 50 },
      transparency: 0.9,
      auto_hide: true,
      always_on_top: true,
    });
  };

  // Demonstrate agent settings updates
  const updateAgentSettings = async () => {
    await updateAgent({
      mode: settings?.agent.mode === "tap" ? "hold" : "tap",
      auto_execute: !settings?.agent.auto_execute,
      confirmation_required: !settings?.agent.confirmation_required,
      max_iterations: settings?.agent.max_iterations === 10 ? 20 : 10,
      timeout_seconds: 300,
      voice_enabled: !settings?.agent.voice_enabled,
    });
  };

  // Demonstrate cloud settings updates
  const updateCloudSettings = async () => {
    await updateCloud({
      enabled: !settings?.cloud.enabled,
      server_url: "wss://example.com/ws",
      device_name: "Updated Device",
      auto_connect: !settings?.cloud.auto_connect,
      security_level:
        settings?.cloud.security_level === "low" ? "medium" : "low",
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
        <span className="ml-3 text-gray-600">Loading settings...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 bg-red-50 border border-red-200 rounded-lg">
        <h3 className="text-red-800 font-medium">Settings Error</h3>
        <p className="text-red-600 mt-1">{error}</p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto p-6 bg-white rounded-lg shadow-lg">
      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold text-gray-900">
          🔧 Centralized Settings Manager Demo
        </h1>
        {onClose && (
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-700"
          >
            ✕
          </button>
        )}
      </div>

      {/* Migration Status */}
      {migrationStatus && (
        <div className="mb-4 p-3 bg-blue-50 border border-blue-200 rounded-lg">
          <p className="text-blue-800">{migrationStatus}</p>
        </div>
      )}

      {/* Tab Navigation */}
      <div className="flex space-x-1 mb-6 bg-gray-100 p-1 rounded-lg">
        {[
          {
            id: "keyboard",
            label: "⌨️ Keyboard",
            count: Object.keys(settings?.keyboard_shortcuts || {}).length,
          },
          {
            id: "floating",
            label: "📱 Floating Bar",
            count: settings?.floating_bar.enabled ? 1 : 0,
          },
          {
            id: "agent",
            label: "🤖 Agent",
            count: settings?.agent.voice_enabled ? 1 : 0,
          },
          {
            id: "cloud",
            label: "☁️ Cloud",
            count: settings?.cloud.enabled ? 1 : 0,
          },
          {
            id: "audio",
            label: "🔊 Audio",
            count: settings?.audio.enabled ? 1 : 0,
          },
          {
            id: "ui",
            label: "🎨 UI",
            count: Object.keys(settings?.ui || {}).length,
          },
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? "bg-white text-blue-600 shadow-sm"
                : "text-gray-600 hover:text-gray-900"
            }`}
          >
            {tab.label}
            <span className="ml-1 text-xs bg-gray-200 px-1 rounded">
              {tab.count}
            </span>
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div className="bg-gray-50 rounded-lg p-6">
        {activeTab === "keyboard" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">Keyboard Shortcuts</h3>
            <div className="grid grid-cols-2 gap-4 mb-4">
              {Object.entries(settings?.keyboard_shortcuts || {}).map(
                ([key, value]) => (
                  <div
                    key={key}
                    className="flex justify-between items-center p-3 bg-white rounded border"
                  >
                    <span className="font-medium">
                      {key.replace(/_/g, " ")}
                    </span>
                    <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                      {value}
                    </code>
                  </div>
                )
              )}
            </div>
            <button
              onClick={updateShortcuts}
              className="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600"
            >
              Update Shortcuts
            </button>
          </div>
        )}

        {activeTab === "floating" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">
              Floating Bar Configuration
            </h3>
            <div className="space-y-3 mb-4">
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Enabled</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.floating_bar.enabled
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.floating_bar.enabled ? "Yes" : "No"}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Position</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  x: {settings?.floating_bar.position.x}, y:{" "}
                  {settings?.floating_bar.position.y}
                </code>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Transparency</span>
                <span className="text-sm">
                  {(settings?.floating_bar.transparency || 0) * 100}%
                </span>
              </div>
            </div>
            <button
              onClick={updateFloatingBarSettings}
              className="bg-purple-500 text-white px-4 py-2 rounded hover:bg-purple-600"
            >
              Toggle Floating Bar
            </button>
          </div>
        )}

        {activeTab === "agent" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">Agent Configuration</h3>
            <div className="space-y-3 mb-4">
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Mode</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  {settings?.agent.mode}
                </code>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Auto Execute</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.agent.auto_execute
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.agent.auto_execute ? "Yes" : "No"}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Max Iterations</span>
                <span className="text-sm">
                  {settings?.agent.max_iterations}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Voice Enabled</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.agent.voice_enabled
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.agent.voice_enabled ? "Yes" : "No"}
                </span>
              </div>
            </div>
            <button
              onClick={updateAgentSettings}
              className="bg-green-500 text-white px-4 py-2 rounded hover:bg-green-600"
            >
              Update Agent Settings
            </button>
          </div>
        )}

        {activeTab === "cloud" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">Cloud Configuration</h3>
            <div className="space-y-3 mb-4">
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Enabled</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.cloud.enabled
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.cloud.enabled ? "Yes" : "No"}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Server URL</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm truncate max-w-xs">
                  {settings?.cloud.server_url}
                </code>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Device Name</span>
                <span className="text-sm">{settings?.cloud.device_name}</span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Security Level</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  {settings?.cloud.security_level}
                </code>
              </div>
            </div>
            <button
              onClick={updateCloudSettings}
              className="bg-indigo-500 text-white px-4 py-2 rounded hover:bg-indigo-600"
            >
              Toggle Cloud Settings
            </button>
          </div>
        )}

        {activeTab === "audio" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">Audio Settings</h3>
            <div className="space-y-3 mb-4">
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Enabled</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.audio.enabled
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.audio.enabled ? "Yes" : "No"}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Voice Provider</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  {settings?.audio.voice_provider}
                </code>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Volume</span>
                <span className="text-sm">
                  {(settings?.audio.volume || 0) * 100}%
                </span>
              </div>
            </div>
          </div>
        )}

        {activeTab === "ui" && (
          <div>
            <h3 className="text-lg font-semibold mb-4">UI Settings</h3>
            <div className="space-y-3 mb-4">
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Theme</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  {settings?.ui.theme}
                </code>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Animations</span>
                <span
                  className={`px-2 py-1 rounded text-sm ${
                    settings?.ui.animations_enabled
                      ? "bg-green-100 text-green-800"
                      : "bg-red-100 text-red-800"
                  }`}
                >
                  {settings?.ui.animations_enabled ? "Yes" : "No"}
                </span>
              </div>
              <div className="flex justify-between items-center p-3 bg-white rounded border">
                <span>Language</span>
                <code className="bg-gray-100 px-2 py-1 rounded text-sm">
                  {settings?.ui.language}
                </code>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Action Buttons */}
      <div className="mt-6 flex flex-wrap gap-3">
        <button
          onClick={handleMigration}
          className="bg-yellow-500 text-white px-4 py-2 rounded hover:bg-yellow-600"
        >
          🔄 Migrate Legacy Settings
        </button>

        <button
          onClick={() => resetSection("keyboard_shortcuts")}
          className="bg-orange-500 text-white px-4 py-2 rounded hover:bg-orange-600"
        >
          Reset Keyboard Shortcuts
        </button>

        <button
          onClick={() => resetAll()}
          className="bg-red-500 text-white px-4 py-2 rounded hover:bg-red-600"
        >
          ⚠️ Reset All Settings
        </button>
      </div>

      {/* Real-time Status */}
      <div className="mt-6 p-4 bg-gray-100 rounded-lg">
        <h4 className="font-medium text-gray-900 mb-2">📊 Real-time Status</h4>
        <div className="grid grid-cols-3 gap-4 text-sm">
          <div>
            <span className="text-gray-600">Settings Loaded:</span>
            <span className="ml-2 text-green-600">✅ Yes</span>
          </div>
          <div>
            <span className="text-gray-600">Auto-sync:</span>
            <span className="ml-2 text-green-600">✅ Active</span>
          </div>
          <div>
            <span className="text-gray-600">Last Update:</span>
            <span className="ml-2 text-gray-900">Just now</span>
          </div>
        </div>
        <p className="text-xs text-gray-500 mt-2">
          💡 All changes are automatically synced across all app components in
          real-time
        </p>
      </div>
    </div>
  );
};
