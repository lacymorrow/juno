import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area"; // To handle potentially large JSON

const DevToolsPanel: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(
    null
  );
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<
    string | null
  >(null);
  const [isLoadingScreenshot, setIsLoadingScreenshot] = useState(false);
  const [isLoadingFocusInfo, setIsLoadingFocusInfo] = useState(false);
  const [isDelayingFocusInfo, setIsDelayingFocusInfo] = useState(false); // New state for delay
  const [isLoadingElementScreenshot, setIsLoadingElementScreenshot] =
    useState(false);
  const [error, setError] = useState<string | null>(null);
  const delayTimeoutRef = useRef<NodeJS.Timeout | null>(null); // Ref to store timeout ID

  // Cleanup timeout on component unmount
  useEffect(() => {
    return () => {
      if (delayTimeoutRef.current) {
        clearTimeout(delayTimeoutRef.current);
      }
    };
  }, []);

  const handleCaptureScreenshot = async () => {
    setIsLoadingScreenshot(true);
    setError(null);
    setScreenshotSrc(null); // Clear previous screenshot
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke("capture_screenshot_command");
      setScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture screenshot:", err);
      setError(`Failed to capture screenshot: ${err?.message || err}`);
    } finally {
      setIsLoadingScreenshot(false);
    }
  };

  const handleCaptureElementScreenshot = async () => {
    setIsLoadingElementScreenshot(true);
    setError(null);
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke(
        "capture_element_screenshot_command"
      );
      setElementScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture element screenshot:", err);
      setError(`Failed to capture element screenshot: ${err?.message || err}`);
    } finally {
      setIsLoadingElementScreenshot(false);
    }
  };

  const fetchAndSetFocusInfo = async () => {
    setIsLoadingFocusInfo(true);
    setElementScreenshotSrc(null);
    try {
      const infoJsonString: string = await invoke(
        "dev_get_focused_element_info"
      );
      // Attempt to parse and re-stringify for pretty printing
      try {
        const parsedInfo = JSON.parse(infoJsonString);
        setFocusedElementInfo(JSON.stringify(parsedInfo, null, 2));
      } catch (parseError) {
        console.error("Failed to parse focus info JSON:", parseError);
        setFocusedElementInfo(infoJsonString); // Show raw string if parsing fails
      }
    } catch (err: any) {
      console.error("Failed to get focused element info:", err);
      setError(`Failed to get focused element info: ${err?.message || err}`);
    } finally {
      setIsLoadingFocusInfo(false);
    }
  };

  const handleGetFocusInfo = async () => {
    setError(null);
    setFocusedElementInfo(null); // Clear previous info
    await fetchAndSetFocusInfo();
  };

  const handleGetFocusInfoWithDelay = async () => {
    if (isDelayingFocusInfo) return; // Prevent multiple clicks

    setError(null);
    setFocusedElementInfo(null); // Clear previous info
    setIsDelayingFocusInfo(true);

    delayTimeoutRef.current = setTimeout(async () => {
      await fetchAndSetFocusInfo();
      setIsDelayingFocusInfo(false);
      delayTimeoutRef.current = null;
    }, 5000); // 5-second delay
  };

  return (
    <Card className="w-full max-w-2xl mx-auto my-4">
      <CardHeader>
        <CardTitle>Developer Tools: Vision Context</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <div
            className="p-4 mb-4 text-sm text-red-800 rounded-lg bg-red-50 dark:bg-gray-800 dark:text-red-400"
            role="alert"
          >
            <span className="font-medium">Error:</span> {error}
          </div>
        )}
        <div className="flex space-x-4 flex-wrap gap-y-2">
          {" "}
          {/* Added flex-wrap and gap-y */}
          <Button
            onClick={handleCaptureScreenshot}
            disabled={isLoadingScreenshot || isDelayingFocusInfo}
          >
            {isLoadingScreenshot ? "Capturing..." : "Capture Screenshot"}
          </Button>
          <Button
            onClick={handleGetFocusInfo}
            disabled={isLoadingFocusInfo || isDelayingFocusInfo}
          >
            {isLoadingFocusInfo
              ? "Getting Info..."
              : "Get Focused Element Info (Now)"}
          </Button>
          <Button
            onClick={handleGetFocusInfoWithDelay}
            disabled={
              isDelayingFocusInfo || isLoadingFocusInfo || isLoadingScreenshot
            }
          >
            {isDelayingFocusInfo
              ? "Waiting 5s... (Switch Focus Now!)"
              : "Get Focused Element Info (After 5s Delay)"}
          </Button>
          <Button
            onClick={handleCaptureElementScreenshot}
            disabled={
              isLoadingElementScreenshot ||
              isDelayingFocusInfo ||
              isLoadingFocusInfo ||
              isLoadingScreenshot
            }
          >
            {isLoadingElementScreenshot
              ? "Capturing Element..."
              : "Capture Focused Element Screenshot"}
          </Button>
        </div>

        {screenshotSrc && (
          <div className="mt-4 border rounded-lg p-2">
            <h3 className="text-lg font-semibold mb-2">Screenshot Preview:</h3>
            <img
              src={screenshotSrc}
              alt="Captured Screenshot"
              className="max-w-full h-auto border"
            />
          </div>
        )}

        {elementScreenshotSrc && (
          <div className="mt-4 border rounded-lg p-2">
            <h3 className="text-lg font-semibold mb-2">
              Focused Element Screenshot:
            </h3>
            <img
              src={elementScreenshotSrc}
              alt="Captured Element Screenshot"
              className="max-w-full h-auto border"
            />
          </div>
        )}

        {focusedElementInfo && (
          <div className="mt-4 border rounded-lg p-2">
            <h3 className="text-lg font-semibold mb-2">
              Focused Element Info:
            </h3>
            <ScrollArea className="h-72 w-full rounded-md border p-4">
              <pre className="text-sm whitespace-pre-wrap break-words">
                <code>{focusedElementInfo}</code>
              </pre>
            </ScrollArea>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

export default DevToolsPanel;
