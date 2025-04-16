import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area"; // To handle potentially large JSON
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";

// Helper type for tracking loading states
type LoadingStates = {
  screenshot: boolean;
  focusInfo: boolean;
  focusDelay: boolean;
  elementScreenshot: boolean;
  clickFocus: boolean;
  typeText: boolean;
  pressKey: boolean;
  openApp: boolean;
  openUrl: boolean;
  scroll: boolean;
};

const DevToolsPanel: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(
    null
  );
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<
    string | null
  >(null);
  const [loadingStates, setLoadingStates] = useState<LoadingStates>({
    screenshot: false,
    focusInfo: false,
    focusDelay: false,
    elementScreenshot: false,
    clickFocus: false,
    typeText: false,
    pressKey: false,
    openApp: false,
    openUrl: false,
    scroll: false,
  });
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null); // For success/status messages

  // Input states
  const [textToType, setTextToType] = useState<string>("Hello from DevTools!");
  const [keyToPress, setKeyToPress] = useState<string>("Return"); // e.g., Return, Tab, cmd+s
  const [appToOpen, setAppToOpen] = useState<string>("TextEdit");
  const [urlToOpen, setUrlToOpen] = useState<string>("https://www.google.com");

  const delayTimeoutRef = useRef<NodeJS.Timeout | null>(null); // Ref to store timeout ID

  // Cleanup timeout on component unmount
  useEffect(() => {
    return () => {
      if (delayTimeoutRef.current) {
        clearTimeout(delayTimeoutRef.current);
      }
    };
  }, []);

  // Generic handler to invoke a command and update loading/status/error
  const invokeCommand = async (
    command: string,
    args?: any,
    loadingKey?: keyof LoadingStates
  ) => {
    if (loadingKey && loadingStates[loadingKey]) return; // Prevent concurrent calls for the same key
    setError(null);
    setStatusMessage(null);
    if (loadingKey) {
      setLoadingStates((prev) => ({ ...prev, [loadingKey]: true }));
    }

    try {
      const result: string = await invoke(command, args);
      setStatusMessage(result); // Show success/status message from backend
    } catch (err: any) {
      console.error(`Failed to invoke ${command}:`, err);
      setError(`Failed command '${command}': ${err?.message || err}`);
    } finally {
      if (loadingKey) {
        setLoadingStates((prev) => ({ ...prev, [loadingKey]: false }));
      }
    }
  };

  const handleCaptureScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, screenshot: true }));
    setError(null);
    setScreenshotSrc(null); // Clear previous screenshot
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke("capture_screenshot_command");
      setScreenshotSrc(`data:image/png;base64,${base64String}`);
      setStatusMessage("Screenshot captured.");
    } catch (err: any) {
      console.error("Failed to capture screenshot:", err);
      setError(`Failed to capture screenshot: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, screenshot: false }));
    }
  };

  const handleCaptureElementScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, elementScreenshot: true }));
    setError(null);
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke(
        "capture_element_screenshot_command"
      );
      setElementScreenshotSrc(`data:image/png;base64,${base64String}`);
      setStatusMessage("Element screenshot captured.");
    } catch (err: any) {
      console.error("Failed to capture element screenshot:", err);
      setError(`Failed to capture element screenshot: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, elementScreenshot: false }));
    }
  };

  const fetchAndSetFocusInfo = async () => {
    setLoadingStates((prev) => ({ ...prev, focusInfo: true }));
    setElementScreenshotSrc(null);
    setStatusMessage(null); // Clear status on new fetch
    try {
      const infoJsonString: string = await invoke(
        "dev_get_focused_element_info"
      );
      setStatusMessage("Focused element info retrieved.");
      // Attempt to parse and re-stringify for pretty printing
      try {
        const parsedInfo = JSON.parse(infoJsonString);
        setFocusedElementInfo(JSON.stringify(parsedInfo, null, 2));
      } catch (parseError) {
        console.error("Failed to parse focus info JSON:", parseError);
        setFocusedElementInfo(infoJsonString); // Show raw string if parsing fails
        setError("Received focus info, but failed to parse as JSON.");
      }
    } catch (err: any) {
      console.error("Failed to get focused element info:", err);
      setError(`Failed to get focused element info: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, focusInfo: false }));
    }
  };

  const handleGetFocusInfo = async () => {
    setError(null);
    setStatusMessage(null);
    setFocusedElementInfo(null); // Clear previous info
    await fetchAndSetFocusInfo();
  };

  const handleGetFocusInfoWithDelay = async () => {
    if (loadingStates.focusDelay) return; // Prevent multiple clicks

    setError(null);
    setStatusMessage(null);
    setFocusedElementInfo(null); // Clear previous info
    setLoadingStates((prev) => ({ ...prev, focusDelay: true }));

    setStatusMessage("Waiting 5s... Switch focus now!"); // Inform user

    delayTimeoutRef.current = setTimeout(async () => {
      await fetchAndSetFocusInfo();
      setLoadingStates((prev) => ({ ...prev, focusDelay: false }));
      delayTimeoutRef.current = null;
    }, 5000); // 5-second delay
  };

  // Specific handlers using invokeCommand
  const handleClickFocused = () =>
    invokeCommand("dev_click_focused_element", {}, "clickFocus");
  const handleTypeText = () =>
    invokeCommand("dev_type_text", { text: textToType }, "typeText");
  const handlePressKey = () =>
    invokeCommand("dev_press_key", { key: keyToPress }, "pressKey");
  const handleOpenApp = () =>
    invokeCommand("dev_open_application", { appName: appToOpen }, "openApp");
  const handleOpenUrl = () =>
    invokeCommand("dev_open_url", { url: urlToOpen }, "openUrl");
  const handleScroll = (direction: "up" | "down") =>
    invokeCommand("dev_scroll_window", { direction }, "scroll");

  return (
    <Card className="w-full max-w-2xl mx-auto my-4">
      <CardHeader>
        <CardTitle>Developer Tools</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <div
            className="p-3 mb-4 text-sm text-red-800 rounded-lg bg-red-50 dark:bg-gray-800 dark:text-red-400"
            role="alert"
          >
            <span className="font-medium">Error:</span> {error}
          </div>
        )}
        {statusMessage && (
          <div
            className="p-3 mb-4 text-sm text-green-800 rounded-lg bg-green-50 dark:bg-gray-800 dark:text-green-400"
            role="status"
          >
            <span className="font-medium">Status:</span> {statusMessage}
          </div>
        )}

        {/* Vision Context Section */}
        <h3 className="text-lg font-semibold border-b pb-2">Vision Context</h3>
        <div className="flex space-x-2 flex-wrap gap-y-2">
          <Button
            onClick={handleCaptureScreenshot}
            disabled={loadingStates.screenshot || loadingStates.focusDelay}
          >
            {loadingStates.screenshot ? "Capturing..." : "Capture Screenshot"}
          </Button>
          <Button
            onClick={handleGetFocusInfo}
            disabled={loadingStates.focusInfo || loadingStates.focusDelay}
          >
            {loadingStates.focusInfo
              ? "Getting Info..."
              : "Get Focused Element Info (Now)"}
          </Button>
          <Button
            onClick={handleGetFocusInfoWithDelay}
            disabled={
              loadingStates.focusDelay ||
              loadingStates.focusInfo ||
              loadingStates.screenshot
            }
          >
            {loadingStates.focusDelay
              ? "Waiting 5s... (Switch Focus Now!)"
              : "Get Focused Element Info (After 5s Delay)"}
          </Button>
          <Button
            onClick={handleCaptureElementScreenshot}
            disabled={
              loadingStates.elementScreenshot ||
              loadingStates.focusDelay ||
              loadingStates.focusInfo ||
              loadingStates.screenshot
            }
          >
            {loadingStates.elementScreenshot
              ? "Capturing Element..."
              : "Capture Focused Element Screenshot"}
          </Button>
        </div>

        {focusedElementInfo && (
          <div className="mt-4 border rounded-lg p-2">
            <h4 className="text-md font-semibold mb-2">
              Focused Element Info:
            </h4>
            <ScrollArea className="h-40 w-full rounded-md border p-4">
              <pre className="text-sm whitespace-pre-wrap break-words">
                <code>{focusedElementInfo}</code>
              </pre>
            </ScrollArea>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {screenshotSrc && (
            <div className="mt-4 border rounded-lg p-2">
              <h4 className="text-md font-semibold mb-2">
                Screenshot Preview:
              </h4>
              <img
                src={screenshotSrc}
                alt="Captured Screenshot"
                className="max-w-full h-auto border"
              />
            </div>
          )}

          {elementScreenshotSrc && (
            <div className="mt-4 border rounded-lg p-2">
              <h4 className="text-md font-semibold mb-2">
                Focused Element Screenshot:
              </h4>
              <img
                src={elementScreenshotSrc}
                alt="Captured Element Screenshot"
                className="max-w-full h-auto border"
              />
            </div>
          )}
        </div>

        <Separator className="my-6" />

        {/* Interaction Section */}
        <h3 className="text-lg font-semibold border-b pb-2">Interactions</h3>

        {/* Click Focused */}
        <div className="flex items-center space-x-2">
          <Button
            onClick={handleClickFocused}
            disabled={loadingStates.clickFocus}
            variant="outline"
          >
            {loadingStates.clickFocus ? "Clicking..." : "Click Focused Element"}
          </Button>
          <p className="text-sm text-muted-foreground">
            Clicks the element currently focused by the OS.
          </p>
        </div>

        {/* Type Text */}
        <div className="flex items-center space-x-2">
          <Button
            onClick={handleTypeText}
            disabled={loadingStates.typeText}
            variant="outline"
          >
            {loadingStates.typeText ? "Typing..." : "Type Text"}
          </Button>
          <Input
            id="text-to-type"
            value={textToType}
            onChange={(e) => setTextToType(e.target.value)}
            className="max-w-sm"
          />
        </div>

        {/* Press Key */}
        <div className="flex items-center space-x-2">
          <Button
            onClick={handlePressKey}
            disabled={loadingStates.pressKey}
            variant="outline"
          >
            {loadingStates.pressKey ? "Pressing..." : "Press Key"}
          </Button>
          <Input
            id="key-to-press"
            value={keyToPress}
            onChange={(e) => setKeyToPress(e.target.value)}
            className="max-w-xs"
            placeholder="e.g., Return, Tab, a, cmd+s"
          />
        </div>

        {/* Scroll Window */}
        <div className="flex items-center space-x-2">
          <Label>Scroll Window:</Label>
          <Button
            onClick={() => handleScroll("up")}
            disabled={loadingStates.scroll}
            variant="outline"
            size="sm"
          >
            {loadingStates.scroll ? "..." : "Up"}
          </Button>
          <Button
            onClick={() => handleScroll("down")}
            disabled={loadingStates.scroll}
            variant="outline"
            size="sm"
          >
            {loadingStates.scroll ? "..." : "Down"}
          </Button>
          <p className="text-sm text-muted-foreground">
            Scrolls the focused window.
          </p>
        </div>

        <Separator className="my-6" />

        {/* Application/URL Section */}
        <h3 className="text-lg font-semibold border-b pb-2">
          App / URL Control
        </h3>

        {/* Open Application */}
        <div className="flex items-center space-x-2">
          <Button
            onClick={handleOpenApp}
            disabled={loadingStates.openApp}
            variant="outline"
          >
            {loadingStates.openApp ? "Opening..." : "Open Application"}
          </Button>
          <Input
            id="app-to-open"
            value={appToOpen}
            onChange={(e) => setAppToOpen(e.target.value)}
            className="max-w-xs"
            placeholder="e.g., TextEdit, Calculator"
          />
        </div>

        {/* Open URL */}
        <div className="flex items-center space-x-2">
          <Button
            onClick={handleOpenUrl}
            disabled={loadingStates.openUrl}
            variant="outline"
          >
            {loadingStates.openUrl ? "Opening..." : "Open URL"}
          </Button>
          <Input
            id="url-to-open"
            type="url"
            value={urlToOpen}
            onChange={(e) => setUrlToOpen(e.target.value)}
            className="max-w-sm"
            placeholder="https://..."
          />
        </div>
      </CardContent>
    </Card>
  );
};

export default DevToolsPanel;
