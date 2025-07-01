/**
 * Test component for the standardized UI API
 * Demonstrates how to use the UI API with a floating element
 */

import React, { useState } from "react";
import { useUIElement, UIElementType, UIState } from "@/lib/ui-api";

interface UIAPITestProps {
  elementType: UIElementType;
  elementId: string;
}

export const UIAPITest: React.FC<UIAPITestProps> = ({
  elementType,
  elementId,
}) => {
  const { manager, state, config } = useUIElement(elementId, elementType);

  const [testMessage, setTestMessage] = useState<string>("");

  // Test basic functionality
  const handleTestClick = async () => {
    if (!manager) return;

    setTestMessage("Testing click interaction...");
    const success = await manager.click({ testData: "click-test" });
    setTestMessage(success ? "✅ Click successful" : "❌ Click failed");
  };

  const handleTestFocus = async () => {
    if (!manager) return;

    setTestMessage("Testing focus interaction...");
    const success = await manager.focus();
    setTestMessage(success ? "✅ Focus successful" : "❌ Focus failed");
  };

  const handleTestInput = async () => {
    if (!manager) return;

    setTestMessage("Testing input interaction...");
    const success = await manager.input("test input value");
    setTestMessage(success ? "✅ Input successful" : "❌ Input failed");
  };

  const handleTestSubmit = async () => {
    if (!manager) return;

    setTestMessage("Testing submit interaction...");
    const success = await manager.submit("test query");
    setTestMessage(success ? "✅ Submit successful" : "❌ Submit failed");
  };

  const handleTestStateUpdate = async () => {
    if (!manager) return;

    setTestMessage("Testing state update...");
    const success = await manager.setState({
      uiState: "loading" as UIState,
      inputValue: "test value",
      transcriptionText: "test transcription",
    });
    setTestMessage(
      success ? "✅ State update successful" : "❌ State update failed"
    );
  };

  const handleTestConfigUpdate = async () => {
    if (!manager) return;

    setTestMessage("Testing config update...");
    const success = await manager.setConfig({
      showVoiceIndicator: !config?.showVoiceIndicator,
      enableAnimations: !config?.enableAnimations,
      opacity: config?.opacity === 0.95 ? 0.8 : 0.95,
    });
    setTestMessage(
      success ? "✅ Config update successful" : "❌ Config update failed"
    );
  };

  const handleTestWindowResize = async () => {
    if (!manager) return;

    setTestMessage("Testing window resize...");
    const success = await manager.resizeWindow(800, 600);
    setTestMessage(
      success ? "✅ Window resize successful" : "❌ Window resize failed"
    );
  };

  const handleTestWindowMove = async () => {
    if (!manager) return;

    setTestMessage("Testing window move...");
    const success = await manager.moveWindow(100, 100);
    setTestMessage(
      success ? "✅ Window move successful" : "❌ Window move failed"
    );
  };

  // The UI API doesn't provide loading/error states at the hook level
  // Instead, state loading is handled internally and errors are reflected in state.currentError

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <h2 className="text-2xl font-bold mb-6">
        UI API Test - {elementType} ({elementId})
      </h2>

      {/* Test Status */}
      <div className="mb-6 p-4 bg-gray-100 rounded-lg">
        <h3 className="font-semibold mb-2">Test Status</h3>
        <p
          className={`text-sm ${
            testMessage.includes("✅")
              ? "text-green-600"
              : testMessage.includes("❌")
              ? "text-red-600"
              : "text-blue-600"
          }`}
        >
          {testMessage || "Ready for testing"}
        </p>
      </div>

      {/* Current State */}
      <div className="mb-6 p-4 bg-blue-50 rounded-lg">
        <h3 className="font-semibold mb-2">Current State</h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <strong>UI State:</strong> {state?.uiState || "N/A"}
          </div>
          <div>
            <strong>Voice Mode:</strong> {state?.voiceMode || "N/A"}
          </div>
          <div>
            <strong>Agent Status:</strong> {state?.agentState || "N/A"}
          </div>
          <div>
            <strong>Input Value:</strong> {state?.inputValue || "N/A"}
          </div>
          <div>
            <strong>Transcription:</strong> {state?.transcriptionText || "N/A"}
          </div>
          <div>
            <strong>Error:</strong> {state?.currentError || "None"}
          </div>
          <div>
            <strong>Agent Working:</strong>{" "}
            {state?.isAgentWorking ? "Yes" : "No"}
          </div>
          <div>
            <strong>Dictation Mode:</strong>{" "}
            {state?.isDictationMode ? "Yes" : "No"}
          </div>
          <div>
            <strong>Always Listening:</strong>{" "}
            {state?.isAlwaysListening ? "Yes" : "No"}
          </div>
          <div>
            <strong>Audio Level:</strong> {state?.audioLevel || 0}
          </div>
        </div>
      </div>

      {/* Current Config */}
      <div className="mb-6 p-4 bg-green-50 rounded-lg">
        <h3 className="font-semibold mb-2">Current Config</h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <strong>Show Voice Indicator:</strong>{" "}
            {config?.showVoiceIndicator ? "Yes" : "No"}
          </div>
          <div>
            <strong>Enable Animations:</strong>{" "}
            {config?.enableAnimations ? "Yes" : "No"}
          </div>
          <div>
            <strong>Auto Hide:</strong> {config?.autoHide ? "Yes" : "No"}
          </div>
          <div>
            <strong>Auto Hide Delay:</strong> {config?.autoHideDelay || 0}ms
          </div>
          <div>
            <strong>Opacity:</strong> {config?.opacity || 0}
          </div>
          <div>
            <strong>Click Through:</strong>{" "}
            {config?.clickThrough ? "Yes" : "No"}
          </div>
          <div>
            <strong>Always On Top:</strong> {config?.alwaysOnTop ? "Yes" : "No"}
          </div>
          <div>
            <strong>Position:</strong>{" "}
            {config?.position
              ? `${config.position.x}, ${config.position.y}`
              : "N/A"}
          </div>
          <div>
            <strong>Dimensions:</strong>{" "}
            {config?.dimensions
              ? `${config.dimensions.width}x${config.dimensions.height}`
              : "N/A"}
          </div>
        </div>
      </div>

      {/* Interaction Tests */}
      <div className="mb-6">
        <h3 className="font-semibold mb-4">Interaction Tests</h3>
        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={handleTestClick}
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
          >
            Test Click
          </button>
          <button
            onClick={handleTestFocus}
            className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600"
          >
            Test Focus
          </button>
          <button
            onClick={handleTestInput}
            className="px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600"
          >
            Test Input
          </button>
          <button
            onClick={handleTestSubmit}
            className="px-4 py-2 bg-orange-500 text-white rounded hover:bg-orange-600"
          >
            Test Submit
          </button>
        </div>
      </div>

      {/* State & Config Tests */}
      <div className="mb-6">
        <h3 className="font-semibold mb-4">State & Config Tests</h3>
        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={handleTestStateUpdate}
            className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600"
          >
            Test State Update
          </button>
          <button
            onClick={handleTestConfigUpdate}
            className="px-4 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600"
          >
            Test Config Update
          </button>
        </div>
      </div>

      {/* Window Tests */}
      <div className="mb-6">
        <h3 className="font-semibold mb-4">Window Management Tests</h3>
        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={handleTestWindowResize}
            className="px-4 py-2 bg-indigo-500 text-white rounded hover:bg-indigo-600"
          >
            Test Window Resize
          </button>
          <button
            onClick={handleTestWindowMove}
            className="px-4 py-2 bg-teal-500 text-white rounded hover:bg-teal-600"
          >
            Test Window Move
          </button>
        </div>
      </div>

      {/* Refresh Controls */}
      <div className="mb-6">
        <h3 className="font-semibold mb-4">Refresh Controls</h3>
        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={() => manager?.getState()}
            className="px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600"
          >
            Refresh State
          </button>
          <button
            onClick={() => manager?.getConfig()}
            className="px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600"
          >
            Refresh Config
          </button>
        </div>
      </div>
    </div>
  );
};

export default UIAPITest;
