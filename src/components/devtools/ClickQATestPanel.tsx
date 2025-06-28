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

// Simple result interface for production testing
interface ProductionTestResult {
  success: boolean;
  operation: string;
  coordinates: [number, number];
  error?: string;
  latency_ms: number;
}

const ClickQATestPanel: React.FC = () => {
  // Individual click test state
  const [clickX, setClickX] = useState<string>("400");
  const [clickY, setClickY] = useState<string>("300");
  const [clickType, setClickType] = useState<string>("left");
  const [clickResult, setClickResult] = useState<ProductionTestResult | null>(
    null
  );

  // Click series test state
  const [seriesPositions, setSeriesPositions] = useState<string>(
    '[[100,100,"left"],[200,200,"right"],[300,300,"double"]]'
  );
  const [seriesResults, setSeriesResults] = useState<
    ProductionTestResult[] | null
  >(null);

  // Coordinate test state
  const [coordX, setCoordX] = useState<string>("500");
  const [coordY, setCoordY] = useState<string>("400");
  const [coordinateResult, setCoordinateResult] = useState<any>(null);

  // Loading states
  const [loading, setLoading] = useState({
    click: false,
    series: false,
    coordinate: false,
  });

  // Production Function Test: Individual Click using actual production functions
  const handleClickTest = async () => {
    const x = parseFloat(clickX);
    const y = parseFloat(clickY);

    if (isNaN(x) || isNaN(y)) {
      toast.error("Please enter valid coordinates");
      return;
    }

    setLoading((prev) => ({ ...prev, click: true }));

    const startTime = Date.now();
    try {
      // Use the actual production mouse functions directly
      let functionName = "";
      switch (clickType) {
        case "left":
          functionName = "left_click";
          await invokeCommand(
            "left_click",
            { x, y, modifier: null },
            "left_click"
          );
          break;
        case "right":
          functionName = "right_click";
          await invokeCommand(
            "right_click",
            { x, y, modifier: null },
            "right_click"
          );
          break;
        case "middle":
          functionName = "middle_click";
          await invokeCommand(
            "middle_click",
            { x, y, modifier: null },
            "middle_click"
          );
          break;
        case "double":
          functionName = "double_click";
          await invokeCommand(
            "double_click",
            { x, y, modifier: null },
            "double_click"
          );
          break;
        case "triple":
          functionName = "triple_click";
          await invokeCommand(
            "triple_click",
            { x, y, modifier: null },
            "triple_click"
          );
          break;
        default:
          throw new Error(`Unknown click type: ${clickType}`);
      }

      const latency_ms = Date.now() - startTime;
      const result: ProductionTestResult = {
        success: true,
        operation: `${clickType} click`,
        coordinates: [x, y],
        latency_ms,
      };

      setClickResult(result);
      toast.success(
        `✅ ${clickType} click successful - Latency: ${latency_ms}ms`
      );
    } catch (error) {
      const latency_ms = Date.now() - startTime;
      const result: ProductionTestResult = {
        success: false,
        operation: `${clickType} click`,
        coordinates: [x, y],
        error: String(error),
        latency_ms,
      };

      setClickResult(result);
      toast.error(`❌ ${clickType} click failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, click: false }));
    }
  };

  // Production Function Test: Click Series using actual production functions
  const handleClickSeriesTest = async () => {
    try {
      const positions = JSON.parse(seriesPositions) as [
        number,
        number,
        string
      ][];
      setLoading((prev) => ({ ...prev, series: true }));

      const results: ProductionTestResult[] = [];

      for (let i = 0; i < positions.length; i++) {
        const [x, y, clickType] = positions[i];
        const startTime = Date.now();

        try {
          // Add delay between clicks
          if (i > 0) {
            await new Promise((resolve) => setTimeout(resolve, 500));
          }

          // Use actual production functions
          switch (clickType) {
            case "left":
              await invokeCommand(
                "left_click",
                { x, y, modifier: null },
                "left_click"
              );
              break;
            case "right":
              await invokeCommand(
                "right_click",
                { x, y, modifier: null },
                "right_click"
              );
              break;
            case "middle":
              await invokeCommand(
                "middle_click",
                { x, y, modifier: null },
                "middle_click"
              );
              break;
            case "double":
              await invokeCommand(
                "double_click",
                { x, y, modifier: null },
                "double_click"
              );
              break;
            case "triple":
              await invokeCommand(
                "triple_click",
                { x, y, modifier: null },
                "triple_click"
              );
              break;
            default:
              throw new Error(`Unknown click type: ${clickType}`);
          }

          const latency_ms = Date.now() - startTime;
          results.push({
            success: true,
            operation: `${clickType} click`,
            coordinates: [x, y],
            latency_ms,
          });
        } catch (error) {
          const latency_ms = Date.now() - startTime;
          results.push({
            success: false,
            operation: `${clickType} click`,
            coordinates: [x, y],
            error: String(error),
            latency_ms,
          });
        }
      }

      setSeriesResults(results);

      const successCount = results.filter((r) => r.success).length;
      const avgLatency =
        results.reduce((sum, r) => sum + r.latency_ms, 0) / results.length;

      toast.success(
        `Series test: ${successCount}/${
          results.length
        } successful, Avg latency: ${avgLatency.toFixed(1)}ms`
      );
    } catch (error) {
      toast.error(`Series test failed: ${error}`);
    } finally {
      setLoading((prev) => ({ ...prev, series: false }));
    }
  };

  // Production Function Test: Mouse Movement and Position Test
  const handleCoordinateTest = async () => {
    const x = parseFloat(coordX);
    const y = parseFloat(coordY);

    if (isNaN(x) || isNaN(y)) {
      toast.error("Please enter valid coordinates");
      return;
    }

    setLoading((prev) => ({ ...prev, coordinate: true }));
    try {
      const startTime = Date.now();

      // Move mouse to target position using production function
      await invokeCommand("mouse_move", { x, y }, "mouse_move");

      // Small delay to ensure movement completes
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Get actual cursor position using production function
      const actualPos = await invokeCommand<[number, number]>(
        "get_cursor_position",
        {},
        "get_cursor_position"
      );

      const latency_ms = Date.now() - startTime;

      const accuracy_error_x = Math.abs(x - actualPos[0]);
      const accuracy_error_y = Math.abs(y - actualPos[1]);
      const is_accurate = accuracy_error_x < 2.0 && accuracy_error_y < 2.0;

      const result = {
        target_coordinates: { x, y },
        actual_coordinates: { x: actualPos[0], y: actualPos[1] },
        accuracy_error: { x: accuracy_error_x, y: accuracy_error_y },
        is_accurate,
        latency_ms,
        move_success: true,
      };

      setCoordinateResult(result);

      const status = is_accurate ? "✅ Accurate" : "❌ Inaccurate";
      toast.info(
        `${status} - Error: X=${accuracy_error_x.toFixed(
          2
        )}px, Y=${accuracy_error_y.toFixed(2)}px`
      );
    } catch (error) {
      toast.error(`Coordinate test failed: ${error}`);
      setCoordinateResult({
        target_coordinates: { x, y },
        actual_coordinates: null,
        accuracy_error: null,
        is_accurate: false,
        move_success: false,
        error: String(error),
      });
    } finally {
      setLoading((prev) => ({ ...prev, coordinate: false }));
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
        <h3 className="text-lg font-semibold">
          Production Mouse Function Testing
        </h3>
        <Badge variant="outline">Using Production Functions</Badge>
      </div>

      {/* Individual Click Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <MousePointer className="h-4 w-4" />
            <span>Individual Click Test</span>
          </CardTitle>
          <CardDescription>
            Test individual mouse clicks using production functions
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-3 gap-2">
            <Input
              placeholder="X coordinate"
              value={clickX}
              onChange={(e) => setClickX(e.target.value)}
              type="number"
            />
            <Input
              placeholder="Y coordinate"
              value={clickY}
              onChange={(e) => setClickY(e.target.value)}
              type="number"
            />
            <select
              value={clickType}
              onChange={(e) => setClickType(e.target.value)}
              className="px-3 py-1 border rounded-md"
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
            {loading.click ? "Testing..." : `Test ${clickType} Click`}
          </Button>

          {clickResult && (
            <div className="p-3 bg-muted rounded-lg space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-medium">{clickResult.operation}</span>
                <Badge
                  variant={clickResult.success ? "default" : "destructive"}
                >
                  {clickResult.success ? "Success" : "Failed"}
                </Badge>
              </div>
              <div className="text-sm text-muted-foreground">
                <p>
                  Coordinates: ({clickResult.coordinates[0]},{" "}
                  {clickResult.coordinates[1]})
                </p>
                <p>Latency: {clickResult.latency_ms}ms</p>
                {clickResult.error && (
                  <p className="text-red-500">Error: {clickResult.error}</p>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Click Series Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <TrendingUp className="h-4 w-4" />
            <span>Click Series Test</span>
          </CardTitle>
          <CardDescription>
            Test multiple clicks in sequence using production functions
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Input
            placeholder='[[100,100,"left"],[200,200,"right"],[300,300,"double"]]'
            value={seriesPositions}
            onChange={(e) => setSeriesPositions(e.target.value)}
          />

          <Button
            onClick={handleClickSeriesTest}
            disabled={loading.series}
            className="w-full"
          >
            {loading.series ? "Testing Series..." : "Test Click Series"}
          </Button>

          {seriesResults && (
            <div className="space-y-2">
              {seriesResults.map((result, index) => (
                <div key={index} className="p-3 bg-muted rounded-lg">
                  <div className="flex items-center justify-between">
                    <span className="font-medium">{result.operation}</span>
                    <Badge variant={result.success ? "default" : "destructive"}>
                      {result.success ? "Success" : "Failed"}
                    </Badge>
                  </div>
                  <div className="text-sm text-muted-foreground">
                    <p>
                      Coordinates: ({result.coordinates[0]},{" "}
                      {result.coordinates[1]})
                    </p>
                    <p>Latency: {result.latency_ms}ms</p>
                    {result.error && (
                      <p className="text-red-500">Error: {result.error}</p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Coordinate Transformation Test */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <Target className="h-4 w-4" />
            <span>Mouse Movement Accuracy Test</span>
          </CardTitle>
          <CardDescription>
            Test mouse movement accuracy using production functions
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-2">
            <Input
              placeholder="X coordinate"
              value={coordX}
              onChange={(e) => setCoordX(e.target.value)}
              type="number"
            />
            <Input
              placeholder="Y coordinate"
              value={coordY}
              onChange={(e) => setCoordY(e.target.value)}
              type="number"
            />
          </div>

          <Button
            onClick={handleCoordinateTest}
            disabled={loading.coordinate}
            className="w-full"
          >
            {loading.coordinate ? "Testing Movement..." : "Test Mouse Movement"}
          </Button>

          {coordinateResult && (
            <div className="p-3 bg-muted rounded-lg space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-medium">Movement Test</span>
                {renderAccuracyBadge(coordinateResult.is_accurate)}
              </div>
              <div className="text-sm text-muted-foreground space-y-1">
                <p>
                  Target: ({coordinateResult.target_coordinates?.x},{" "}
                  {coordinateResult.target_coordinates?.y})
                </p>
                {coordinateResult.actual_coordinates && (
                  <>
                    <p>
                      Actual: ({coordinateResult.actual_coordinates.x},{" "}
                      {coordinateResult.actual_coordinates.y})
                    </p>
                    <p>
                      Error: X={coordinateResult.accuracy_error?.x?.toFixed(2)}
                      px, Y={coordinateResult.accuracy_error?.y?.toFixed(2)}px
                    </p>
                  </>
                )}
                {coordinateResult.latency_ms && (
                  <p>Latency: {coordinateResult.latency_ms}ms</p>
                )}
                {coordinateResult.error && (
                  <p className="text-red-500">
                    Error: {coordinateResult.error}
                  </p>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default ClickQATestPanel;
