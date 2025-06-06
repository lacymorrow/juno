import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Focus, PlayCircle } from 'lucide-react';
import { invokeCommand } from '@/lib/utils';
import type { ClickQAResult, CoordinateTestResult, VisualizationTestResult } from '@/types/devtools';

const ScreenshotOperations: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(null);
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<string | null>(null);
  const [selectorString, setSelectorString] = useState<string>('button:OK');
  const [clickQAResults, setClickQAResults] = useState<ClickQAResult[] | null>(null);
  const [coordinateResults, setCoordinateResults] = useState<CoordinateTestResult[] | null>(null);
  const [visualizationResults, setVisualizationResults] = useState<VisualizationTestResult[] | null>(null);

  const handleTakeScreenshot = async () => {
    const result = await invokeCommand<string | null>(
      'dev_take_screenshot',
      {},
      'screenshot'
    );
    if (result) {
      setScreenshotSrc(result);
    }
  };

  const handleGetFocusedInfo = async () => {
    const result = await invokeCommand<string | null>(
      'dev_get_focused_info',
      {},
      'focusInfo'
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
      'dev_get_element_screenshot',
      { selector: selectorString.trim() },
      'elementScreenshot'
    );
    if (result) {
      setElementScreenshotSrc(result);
    }
  };

  const handleTestClickVisualization = async () => {
    const result = await invokeCommand<ClickQAResult[]>(
      'dev_test_click_visualization',
      {},
      'testClickVisualization'
    );
    setClickQAResults(result);
  };

  const handleTestCoordinateTransformation = async () => {
    const result = await invokeCommand<CoordinateTestResult[]>(
      'dev_test_coordinate_transformation',
      {},
      'testCoordinateTransformation'
    );
    setCoordinateResults(result);
  };

  const handleTestVisualization = async () => {
    const result = await invokeCommand<VisualizationTestResult[]>(
      'dev_test_visualization',
      {},
      'testVisualization'
    );
    setVisualizationResults(result);
  };

  return (
    <div className="space-y-4">
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
          <Button onClick={handleGetFocusedInfo}>Get Focused Element Info</Button>
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
            Test Click Visualization
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