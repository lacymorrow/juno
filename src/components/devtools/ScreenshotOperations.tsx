import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Focus, Info } from "lucide-react";
import { invokeCommand } from "@/lib/utils";

const ScreenshotOperations: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string>("");
  const [focusedElementInfo, setFocusedElementInfo] = useState<string>("");
  const [selectorString, setSelectorString] = useState<string>("");
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<string>("");

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

  return (
    <div className="space-y-4">
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 mb-4">
        <div className="flex items-start space-x-2">
          <Info className="h-4 w-4 text-blue-600 mt-0.5 flex-shrink-0" />
          <div className="text-sm text-blue-800">
            <strong>Production Functions:</strong> This component uses the
            official Anthropic Computer Use API for screenshots and production
            functions for element operations. QA testing is done through actual
            production functions.
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
    </div>
  );
};

export default ScreenshotOperations;
