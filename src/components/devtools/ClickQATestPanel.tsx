import React, { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import {
  MousePointer,
  Target,
  TrendingUp,
  Palette,
  AlertCircle,
  CheckCircle,
} from "lucide-react";
import { invokeCommand } from "@/lib/utils";
import type { ClickQAResult, CoordinateTestResult } from "@/types/devtools";

interface ClickSeriesPosition {
  x: number;
  y: number;
  click_type: string;
}

const ClickQATestPanel: React.FC = () => {
  // Individual click test state
  const [clickX, setClickX] = useState<string>("400");
  const [clickY, setClickY] = useState<string>("300");
  const [clickType, setClickType] = useState<string>("left");
  const [clickResult, setClickResult] = useState<ClickQAResult | null>(null);

  // Click series test state
  const [seriesPositions, setSeriesPositions] = useState<string>(
    '[[100,100,"left"],[200,200,"right"],[300,300,"double"]]'
  );
  const [seriesResults, setSeriesResults] = useState<ClickQAResult[] | null>(
    null
  );

  // Coordinate transformation test state
  const [coordX, setCoordX] = useState<string>("500");
  const [coordY, setCoordY] = useState<string>("400");
  const [coordinateResult, setCoordinateResult] = useState<any>(null);

  // Click visualization test state
  const [visualizationResult, setVisualizationResult] = useState<any>(null);

  // Loading states
  const [loading, setLoading] = useState({
    click: false,
    series: false,
    coordinate: false,
    visualization: false,
  });

  // QA Function 1: Individual Click Test
  const handleClickTest = async () => {
    const x = parseFloat(clickX);
    const y = parseFloat(clickY);

    if (isNaN(x) || isNaN(y)) {
      toast.error("Please enter valid coordinates");
      return;
    }

    setLoading((prev) => ({ ...prev, click: true }));
    try {
      const result = await invokeCommand<ClickQAResult>(
        "qa_test_click",
        { x, y, clickType },
        "qa_test_click"
      );
      setClickResult(result);

      const status = result.success ? "✅ Success" : "❌ Failed";
      const accuracy = result.cursor_position_after
        ? `Accuracy: ${Math.abs(x - result.cursor_position_after[0]).toFixed(
            1
          )}px, ${Math.abs(y - result.cursor_position_after[1]).toFixed(1)}px`
        : "";

      toast.success(
        `${status} - Latency: ${result.latency_ms.toFixed(1)}ms ${accuracy}`
      );
    } catch (error) {
      toast.error(`Click test failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, click: false }));
    }
  };

  // QA Function 2: Click Series Test
  const handleClickSeriesTest = async () => {
    try {
      const positions = JSON.parse(seriesPositions) as [
        number,
        number,
        string
      ][];
      setLoading((prev) => ({ ...prev, series: true }));

      const result = await invokeCommand<ClickQAResult[]>(
        "qa_test_click_series",
        { positions },
        "qa_test_click_series"
      );
      setSeriesResults(result);

      const successCount = result.filter((r) => r.success).length;
      const avgLatency =
        result.reduce((sum, r) => sum + r.latency_ms, 0) / result.length;

      toast.success(
        `Series test: ${successCount}/${
          result.length
        } successful, Avg latency: ${avgLatency.toFixed(1)}ms`
      );
    } catch (error) {
      toast.error(`Series test failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, series: false }));
    }
  };

  // QA Function 3: Coordinate Transformation Test
  const handleCoordinateTest = async () => {
    const x = parseFloat(coordX);
    const y = parseFloat(coordY);

    if (isNaN(x) || isNaN(y)) {
      toast.error("Please enter valid coordinates");
      return;
    }

    setLoading((prev) => ({ ...prev, coordinate: true }));
    try {
      const result = await invokeCommand<any>(
        "qa_test_coordinate_transformation",
        { x, y },
        "qa_test_coordinate_transformation"
      );
      setCoordinateResult(result);

      const isAccurate = result.is_accurate ? "✅ Accurate" : "❌ Inaccurate";
      const errorX = result.accuracy_error?.x?.toFixed(2) || "N/A";
      const errorY = result.accuracy_error?.y?.toFixed(2) || "N/A";

      toast.info(`${isAccurate} - Error: X=${errorX}px, Y=${errorY}px`);
    } catch (error) {
      toast.error(`Coordinate test failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, coordinate: false }));
    }
  };

  // QA Function 4: Click Visualization Test
  const handleVisualizationTest = async () => {
    setLoading((prev) => ({ ...prev, visualization: true }));
    try {
      const result = await invokeCommand<any>(
        "qa_test_click_visualization",
        {},
        "qa_test_click_visualization"
      );
      setVisualizationResult(result);

      const successRate = (result.success_rate * 100).toFixed(1);
      toast.success(`Visualization test: ${successRate}% success rate`);
    } catch (error) {
      toast.error(`Visualization test failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, visualization: false }));
    }
  };

  const renderAccuracyBadge = (isAccurate: boolean | undefined) => {
    if (isAccurate === undefined) return null;
    return (
      <Badge variant={isAccurate ? "default" : "destructive"}>
        {isAccurate ? (
          <CheckCircle className="w-3 h-3 mr-1" />
        ) : (
          <AlertCircle className="w-3 h-3 mr-1" />
        )}
        {isAccurate ? "Accurate" : "Inaccurate"}
      </Badge>
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center space-x-2">
        <Target className="h-5 w-5" />
        <h3 className="text-lg font-semibold">Click System QA Testing</h3>
      </div>

      {/* QA Function 1: Individual Click Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <MousePointer className="h-4 w-4" />
            <span>1. Individual Click Test</span>
          </CardTitle>
          <CardDescription>
            Test click accuracy at specific coordinates with latency measurement
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-3 gap-2">
            <Input
              placeholder="X coordinate"
              value={clickX}
              onChange={(e) => setClickX(e.target.value)}
            />
            <Input
              placeholder="Y coordinate"
              value={clickY}
              onChange={(e) => setClickY(e.target.value)}
            />
            <select
              value={clickType}
              onChange={(e) => setClickType(e.target.value)}
              className="px-3 py-2 border rounded-md"
            >
              <option value="left">Left Click</option>
              <option value="right">Right Click</option>
              <option value="middle">Middle Click</option>
              <option value="double">Double Click</option>
              <option value="triple">Triple Click</option>
            </select>
          </div>
          <Button
            onClick={handleClickTest}
            disabled={loading.click}
            className="w-full"
          >
            {loading.click ? "Testing..." : "Test Click"}
          </Button>

          {clickResult && (
            <div className="mt-4 p-4 bg-gray-50 rounded-lg">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <strong>Success:</strong>{" "}
                  {clickResult.success ? "✅ Yes" : "❌ No"}
                </div>
                <div>
                  <strong>Latency:</strong> {clickResult.latency_ms.toFixed(1)}
                  ms
                </div>
                <div>
                  <strong>Target:</strong> ({clickResult.coordinates[0]},{" "}
                  {clickResult.coordinates[1]})
                </div>
                <div>
                  <strong>Actual:</strong>{" "}
                  {clickResult.cursor_position_after
                    ? `(${clickResult.cursor_position_after[0]}, ${clickResult.cursor_position_after[1]})`
                    : "N/A"}
                </div>
              </div>
              {clickResult.error && (
                <div className="mt-2 text-red-600 text-sm">
                  <strong>Error:</strong> {clickResult.error}
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      {/* QA Function 2: Click Series Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <TrendingUp className="h-4 w-4" />
            <span>2. Click Series Test</span>
          </CardTitle>
          <CardDescription>
            Test multiple clicks in sequence to measure consistency
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <label className="text-sm font-medium">
              Positions (JSON format): [[x,y,"type"], ...]
            </label>
            <Input
              placeholder='[[100,100,"left"],[200,200,"right"],[300,300,"double"]]'
              value={seriesPositions}
              onChange={(e) => setSeriesPositions(e.target.value)}
              className="mt-1"
            />
          </div>
          <Button
            onClick={handleClickSeriesTest}
            disabled={loading.series}
            className="w-full"
          >
            {loading.series ? "Testing..." : "Test Click Series"}
          </Button>

          {seriesResults && (
            <div className="mt-4 p-4 bg-gray-50 rounded-lg">
              <div className="mb-2">
                <strong>Results:</strong>{" "}
                {seriesResults.filter((r) => r.success).length}/
                {seriesResults.length} successful
              </div>
              <div className="space-y-2 max-h-40 overflow-y-auto">
                {seriesResults.map((result, index) => (
                  <div
                    key={index}
                    className="text-xs border-l-2 border-gray-300 pl-2"
                  >
                    <span
                      className={
                        result.success ? "text-green-600" : "text-red-600"
                      }
                    >
                      {index + 1}. {result.operation} -{" "}
                      {result.success ? "✅" : "❌"}(
                      {result.latency_ms.toFixed(1)}ms)
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* QA Function 3: Coordinate Transformation Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <Target className="h-4 w-4" />
            <span>3. Coordinate Transformation Test</span>
          </CardTitle>
          <CardDescription>
            Test coordinate accuracy between different coordinate systems
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-2">
            <Input
              placeholder="X coordinate"
              value={coordX}
              onChange={(e) => setCoordX(e.target.value)}
            />
            <Input
              placeholder="Y coordinate"
              value={coordY}
              onChange={(e) => setCoordY(e.target.value)}
            />
          </div>
          <Button
            onClick={handleCoordinateTest}
            disabled={loading.coordinate}
            className="w-full"
          >
            {loading.coordinate
              ? "Testing..."
              : "Test Coordinate Transformation"}
          </Button>

          {coordinateResult && (
            <div className="mt-4 p-4 bg-gray-50 rounded-lg">
              <div className="flex items-center justify-between mb-2">
                <strong>Coordinate Accuracy Test</strong>
                {renderAccuracyBadge(coordinateResult.is_accurate)}
              </div>
              <div className="grid grid-cols-2 gap-4 text-xs">
                <div>
                  <strong>Original:</strong> (
                  {coordinateResult.original_scaled?.x},{" "}
                  {coordinateResult.original_scaled?.y})
                </div>
                <div>
                  <strong>Screen:</strong> (
                  {coordinateResult.calculated_screen?.x.toFixed(1)},{" "}
                  {coordinateResult.calculated_screen?.y.toFixed(1)})
                </div>
                <div>
                  <strong>Accuracy Error:</strong> X=
                  {coordinateResult.accuracy_error?.x?.toFixed(2)}px, Y=
                  {coordinateResult.accuracy_error?.y?.toFixed(2)}px
                </div>
                <div>
                  <strong>Roundtrip Error:</strong> X=
                  {coordinateResult.roundtrip_error?.x?.toFixed(2)}px, Y=
                  {coordinateResult.roundtrip_error?.y?.toFixed(2)}px
                </div>
              </div>
              <div className="mt-2 text-xs text-gray-600">
                Watch for colored dots: Green=Original, Blue=Roundtrip,
                Red=Actual
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* QA Function 4: Click Visualization Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <Palette className="h-4 w-4" />
            <span>4. Click Visualization Test</span>
          </CardTitle>
          <CardDescription>
            Test visual feedback system with colored click indicators
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Button
            onClick={handleVisualizationTest}
            disabled={loading.visualization}
            className="w-full"
          >
            {loading.visualization ? "Testing..." : "Test Click Visualization"}
          </Button>

          {visualizationResult && (
            <div className="mt-4 p-4 bg-gray-50 rounded-lg">
              <div className="mb-2">
                <strong>Visualization Test Results</strong>
                <Badge variant="default" className="ml-2">
                  {(visualizationResult.success_rate * 100).toFixed(1)}% Success
                  Rate
                </Badge>
              </div>
              <div className="text-xs text-gray-600">
                Watch for colored circles appearing on screen: Red, Green, Blue,
                Yellow, Magenta
              </div>
              {visualizationResult.results && (
                <div className="mt-2 space-y-1 max-h-32 overflow-y-auto">
                  {visualizationResult.results.map(
                    (result: any, index: number) => (
                      <div
                        key={index}
                        className="text-xs flex items-center space-x-2"
                      >
                        <div
                          className="w-3 h-3 rounded-full"
                          style={{ backgroundColor: result.color }}
                        ></div>
                        <span
                          className={
                            result.success ? "text-green-600" : "text-red-600"
                          }
                        >
                          ({result.position.x.toFixed(0)},{" "}
                          {result.position.y.toFixed(0)}) -{" "}
                          {result.success ? "✅" : "❌"}
                        </span>
                      </div>
                    )
                  )}
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      <Separator />

      <div className="text-sm text-gray-600 space-y-2">
        <h4 className="font-medium">Testing Tips:</h4>
        <ul className="list-disc pl-4 space-y-1">
          <li>Test with different screen areas (corners, center, edges)</li>
          <li>Try multiple monitor setups if available</li>
          <li>Watch for systematic patterns in coordinate errors</li>
          <li>Note if errors increase with distance from origin</li>
          <li>Check if errors are consistent across click types</li>
        </ul>
      </div>
    </div>
  );
};

export default ClickQATestPanel;
