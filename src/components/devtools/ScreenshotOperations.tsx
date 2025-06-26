import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Focus, PlayCircle, Info } from "lucide-react";
import { invokeCommand } from "@/lib/utils";
import type {
  ClickQAResult,
  CoordinateTestResult,
  VisualizationTestResult,
} from "@/types/devtools";

const ScreenshotOperations: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(
    null
  );
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<
    string | null
  >(null);
  const [selectorString, setSelectorString] = useState<string>("button:OK");
  const [clickQAResults, setClickQAResults] = useState<ClickQAResult[] | null>(
    null
  );
  const [coordinateResults, setCoordinateResults] = useState<
    CoordinateTestResult[] | null
  >(null);
  const [visualizationResults, setVisualizationResults] = useState<
    VisualizationTestResult[] | null
  >(null);

  const handleTakeScreenshot = async () => {
    const result = await invokeCommand<any>(
      "computer",
      { action: "screenshot" },
      "screenshot"
    );
    if (result && result.base64_image) {
      setScreenshotSrc(`data:image/png;base64,${result.base64_image}`);
    }
  };

  const handleGetFocusedInfo = async () => {
    const result = await invokeCommand<string | null>(
      "get_focused_element_info",
      {},
      "focusInfo"
    );
    if (result) {
      setFocusedElementInfo(result);
    }
  };

  const handleGetElementScreenshot = async () => {
    if (!selectorString.trim()) {
      return;
    }
    const result = await invokeCommand<string | null>(
      "capture_element_screenshot_command",
      { selector: selectorString },
      "elementScreenshot"
    );
    if (result) {
      setElementScreenshotSrc(result);
    }
  };

  const handleTestClickVisualization = async () => {
    try {
      // Test click visualization at center of screen with red color
      await invokeCommand<void>(
        "test_click_visualization",
        { x: 400, y: 300, color: "#FF0000" },
        "testClickVisualization"
      );
      // Since this returns Result<(), String>, we show success
      setClickQAResults([{
        success: true,
        operation: "click_visualization_test",
        coordinates: [400, 300],
        error: undefined,
        visualization_success: true,
        cursor_position_after: undefined,
        latency_ms: 0
      }]);
    } catch (error) {
      // Handle error case
      setClickQAResults([{
        success: false,
        operation: "click_visualization_test",
        coordinates: [400, 300],
        error: String(error),
        visualization_success: false,
        cursor_position_after: undefined,
        latency_ms: 0
      }]);
    }
  };

  const handleTestCoordinateTransformation = async () => {
    const result = await invokeCommand<any>(
      "qa_test_coordinate_transformation",
      { x: 400, y: 300 },
      "testCoordinateTransformation"
    );
    setCoordinateResults([{
      original: result.original_scaled,
      transformed_to_screen: result.calculated_screen,
      transformed_back: result.roundtrip_scaled,
      error: result.roundtrip_error,
      scaling_info: result.scaling_info,
      is_accurate: result.is_accurate
    }]);
  };

  const handleTestVisualization = async () => {
    const result = await invokeCommand<any>(
      "qa_test_click_visualization",
      {},
      "testVisualization"
    );
    setVisualizationResults([result]);
  };

  return (
    <div className="space-y-4">
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 mb-4">
        <div className="flex items-start space-x-2">
          <Info className="h-4 w-4 text-blue-600 mt-0.5 flex-shrink-0" />
          <div className="text-sm text-blue-800">
            <strong>Tool Consolidation:</strong> This component now uses the
            official Anthropic Computer Use API for screenshots and production
            functions for element operations instead of dev_* functions.
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Button onClick={handleTakeScreenshot}>Take Screenshot</Button>
        </div>
        {screenshotSrc && (
          <img
            src={screenshotSrc}
            alt="Screenshot"
            className="max-w-full h-auto"
          />
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Focus className="h-4 w-4" />
          <Button onClick={handleGetFocusedInfo}>
            Get Focused Element Info
          </Button>
        </div>
        {focusedElementInfo && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {focusedElementInfo}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Input
            placeholder="Element selector"
            value={selectorString}
            onChange={(e) => setSelectorString(e.target.value)}
          />
          <Button onClick={handleGetElementScreenshot}>
            Get Element Screenshot
          </Button>
        </div>
        {elementScreenshotSrc && (
          <img
            src={elementScreenshotSrc}
            alt="Element Screenshot"
            className="max-w-full h-auto"
          />
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <PlayCircle className="h-4 w-4" />
          <Button onClick={handleTestClickVisualization}>
            Test Click Visualization (QA)
          </Button>
        </div>
        {clickQAResults && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {JSON.stringify(clickQAResults, null, 2)}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <PlayCircle className="h-4 w-4" />
          <Button onClick={handleTestCoordinateTransformation}>
            Test Coordinate Transformation
          </Button>
        </div>
        {coordinateResults && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {JSON.stringify(coordinateResults, null, 2)}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <PlayCircle className="h-4 w-4" />
          <Button onClick={handleTestVisualization}>Test Visualization</Button>
        </div>
        {visualizationResults && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {JSON.stringify(visualizationResults, null, 2)}
          </pre>
        )}
      </div>
    </div>
  );
};

export default ScreenshotOperations;
